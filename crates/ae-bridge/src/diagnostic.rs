//! User-invoked local diagnostics. Passive inventory is separate from explicit execution.
use crate::{
    ProjectConverter, discovery,
    process::{ExternalConverter, Scratch, Worker, read_bounded},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
pub struct Options {
    pub folders: Vec<String>,
    pub plugins: Vec<PathBuf>,
    pub installed: bool,
    pub execute_matching: bool,
    pub match_name: Option<String>,
    pub self_test: bool,
    pub preset: Option<PathBuf>,
    pub output: PathBuf,
}

fn save(output: &Path, report: &Value) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    if data.len() > 32 * 1024 * 1024 {
        return Err("diagnostic report exceeds 32 MiB".into());
    }
    fs::write(output, data).map_err(|e| format!("cannot save diagnostic log {}: {e}", output.display()))
}

/// Static direct-import hints. Delay imports, API sets, transitive imports, activation
/// contexts, dynamically loaded DLLs and vendor-specific search rules need loader evidence.
pub fn dependency_hints(plugin: &Path, names: &[String]) -> Vec<Value> {
    let mut search = vec![];
    if let Some(parent) = plugin.parent() {
        search.push(parent.to_path_buf());
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        search.push(parent.to_path_buf());
    }
    if let Some(windir) = std::env::var_os("WINDIR") {
        let root = PathBuf::from(windir);
        search.push(root.join("System32"));
        search.push(root);
    }
    if let Some(path) = std::env::var_os("PATH") {
        search.extend(std::env::split_paths(&path).filter(|p| p.is_absolute()).take(128));
    }
    names.iter().take(2048).map(|name| {
        let safe=!name.is_empty() && name.chars().all(|c|c.is_ascii_alphanumeric() || matches!(c,'.'|'_'|'-'));
        let api_set=name.to_ascii_lowercase().starts_with("api-ms-") || name.to_ascii_lowercase().starts_with("ext-ms-");
        let found=if safe && !api_set { search.iter().map(|p|p.join(name)).find(|p|p.is_file()) } else { None };
        json!({"name":name,"status":if !safe {"invalidBasename"}else if api_set {"apiSetResolvedByWindows"}else if found.is_some(){"foundInLimitedSearch"}else{"notFoundInLimitedSearch"},"path":found,"missingConfirmed":false})
    }).collect()
}

fn read_checkpoint(path: &Path) -> Option<Value> {
    read_bounded(path, 1024 * 1024).ok().and_then(|data| serde_json::from_slice(&data).ok())
}

/// The helper process owns the ABI. A partial checkpoint survives errors, exits or timeouts.
pub fn execute_test(worker: &Path, plugin: &Path, time: f64, params: &[(usize, f64)]) -> Result<Value, String> {
    if !worker.is_absolute() || !worker.is_file() || !plugin.is_absolute() || !plugin.is_file() {
        return Err("worker and plugin must be existing absolute paths".into());
    }
    if !time.is_finite() || time.abs() > 2_000_000.0 || params.len() > 256 || params.iter().any(|(_, v)| !v.is_finite()) {
        return Err("invalid diagnostic time/parameters".into());
    }
    let scratch = Scratch::create()?;
    let request = scratch.0.join("request.json");
    let response = scratch.0.join("response.json");
    let output = scratch.0.join("output.rgba32f");
    let log_path = scratch.0.join("worker.log");
    let input: Vec<[f32; 4]> = (0..4096).map(|i| [(i % 64) as f32 / 128.0, (i / 64) as f32 / 128.0, 0.25, 1.0]).collect();
    let data: Vec<_> = input.iter().flat_map(|p| p.iter().flat_map(|v| v.to_le_bytes())).collect();
    fs::write(scratch.0.join("input.rgba32f"), data).map_err(|e| e.to_string())?;
    fs::write(
        &request,
        serde_json::to_vec(&json!({"protocol":1,"operation":"diagnose","plugin":plugin,"width":64,"height":64,"time":time,"params":params}))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut command = Command::new(worker);
    command
        .args([&request, &response])
        .current_dir(&scratch.0)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log));
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = Worker(command.spawn().map_err(|e| format!("cannot launch worker: {e}"))?);
    let start = Instant::now();
    let mut exit = None;
    let mut stopped = None;
    loop {
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            exit = Some(status);
            break;
        }
        if start.elapsed() >= Duration::from_secs(30) {
            stopped = Some("30 second deadline; worker terminated");
            break;
        }
        if [(&response, 1024 * 1024), (&output, 4096 * 16), (&log_path, 1024 * 1024)]
            .iter()
            .any(|(p, limit)| fs::metadata(p).is_ok_and(|m| m.len() > *limit as u64))
        {
            stopped = Some("worker output limit exceeded; worker terminated");
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if stopped.is_some() {
        let _ = child.0.kill();
        let _ = child.0.wait();
    }
    let checkpoint = read_checkpoint(&response);
    let success =
        exit.is_some_and(|s| s.success()) && checkpoint.as_ref().is_some_and(|r| r["protocol"] == 1 && r["rendered"] == true && r["stage"] == "complete");
    let mut frame = json!({"verified":false,"pixelsChanged":false});
    if success {
        match read_bounded(&output, 4096 * 16) {
            Ok(data) if data.len() == 4096 * 16 => {
                let mut changed = 0;
                let mut max_delta = 0f64;
                let mut finite = true;
                let mut gain_error = 0f64;
                for (source, pixel) in input.iter().zip(data.chunks_exact(16)) {
                    let mut different = false;
                    for (c, channel) in pixel.chunks_exact(4).enumerate() {
                        let v = f32::from_le_bytes(channel.try_into().map_err(|_| "invalid output channel")?);
                        finite &= v.is_finite();
                        let original = *source.get(c).ok_or("invalid diagnostic channel")?;
                        let delta = f64::from((v - original).abs());
                        max_delta = max_delta.max(delta);
                        different |= delta > 1e-7;
                        let gain = if c == 3 { 1.0 } else { 0.5 + time * 0.1 };
                        gain_error = gain_error.max((f64::from(v) - f64::from(original) * gain).abs());
                    }
                    if different {
                        changed += 1;
                    }
                }
                frame = json!({"verified":finite,"width":64,"height":64,"pixelsChanged":finite && changed>0,"changedPixels":changed,"maximumChannelDelta":max_delta,"gainFixtureMaximumError":gain_error});
            }
            Ok(_) => frame["error"] = json!("wrong output frame length"),
            Err(e) => frame["error"] = json!(e),
        }
    }
    let log = read_bounded(&log_path, 1024 * 1024).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_else(|e| e);
    Ok(
        json!({"plugin":plugin,"time":time,"requestedFloatParameters":params,"exitCode":exit.and_then(|s|s.code()),"workerExitedSuccessfully":exit.is_some_and(|s|s.success()),"termination":stopped,"elapsedMilliseconds":start.elapsed().as_millis(),"checkpoint":checkpoint,"frame":frame,"log":log,"presetApplied":false,"visualFidelityVerified":false}),
    )
}

fn preset_analysis(path: &Path, converter: PathBuf) -> Result<Value, String> {
    let source = read_bounded(path, 1024 * 1024)?;
    let conversion = ExternalConverter { executable: converter, timeout: Duration::from_secs(30) }.convert(&source)?;
    let project: Value = serde_json::from_str(&conversion.project_json).map_err(|e| e.to_string())?;
    let mut pending = vec![(&project, 0usize)];
    let mut visited = 0usize;
    while let Some((value, depth)) = pending.pop() {
        visited += 1;
        if visited > 100_000 || depth > 32 {
            return Err("preset carrier traversal limit".into());
        }
        match value {
            Value::Object(map) => {
                if let Some(comment) = map.get("comment").and_then(Value::as_str)
                    && comment.len() <= 6 * 1024 * 1024
                    && let Ok(mut metadata) = serde_json::from_str::<Value>(comment)
                    && metadata["effectcraftPreset"] == 1
                {
                    if let Some(map) = metadata.as_object_mut() {
                        map.remove("sourceHex");
                    }
                    return Ok(
                        json!({"source":path,"sourceBytes":source.len(),"decoded":metadata,"conversionReport":serde_json::from_str::<Value>(&conversion.report_json).map_err(|e|e.to_string())?,"appliedToNativePlugin":false,"bindingStatus":"blocked: native disk IDs, types and preset semantics require verified mapping; custom data is not applied"}),
                    );
                }
                pending.extend(map.values().map(|v| (v, depth + 1)));
            }
            Value::Array(values) => pending.extend(values.iter().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    Err("converter produced no FFX metadata carrier".into())
}

pub fn run(options: Options) -> Result<(), String> {
    if options.output.as_os_str().is_empty() || options.folders.len() > 64 || options.plugins.len() > 16 {
        return Err("output path required; at most 64 folders and 16 explicit plugins".into());
    }
    if options.execute_matching && options.match_name.as_deref().is_none_or(|s| s.is_empty() || s.len() > 1024) {
        return Err("--execute-matching requires an exact --match-name".into());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let worker = std::env::var_os("EFFECTCRAFT_AEX_WORKER").map(PathBuf::from).unwrap_or_else(|| executable.with_file_name("effectcraft-aex-worker.exe"));
    let mut folders = options.folders;
    if options.installed {
        folders.extend(discovery::default_folders());
    }
    folders.sort();
    folders.dedup();
    let scan = discovery::scan(&discovery::PluginConfig { folders, ..Default::default() });
    let dependencies:Vec<_>=scan.plugins.iter().map(|p|json!({"plugin":p.path,"directImports":p.metadata.as_ref().map(|m|dependency_hints(Path::new(&p.path),&m.imports)),"delayImports":p.metadata.as_ref().map(|m|dependency_hints(Path::new(&p.path),&m.delay_imports))})).collect();
    let matching: Vec<_> = scan
        .plugins
        .iter()
        .filter(|p| {
            p.metadata.as_ref().is_some_and(|m| m.effects.iter().any(|e| e.kind == "eFKT" && Some(e.match_name.as_str()) == options.match_name.as_deref()))
        })
        .collect();
    let mut targets = options.plugins;
    if options.execute_matching {
        targets.extend(matching.iter().map(|p| PathBuf::from(&p.path)));
    }
    let mut fixture = None;
    if options.self_test {
        let path = executable.with_file_name("test-plugins").join("EffectCraft Gain.aex");
        targets.push(path.clone());
        fixture = Some(fs::canonicalize(&path).unwrap_or(path));
    }
    let mut targets: Vec<_> = targets.into_iter().map(|p| fs::canonicalize(&p).unwrap_or(p)).collect();
    targets.sort();
    targets.dedup();
    if targets.len() > 16 {
        return Err("more than 16 execution targets; select a narrower folder or explicit --plugin".into());
    }
    let mut report = json!({"schema":"effectcraft.aex-diagnostic.1","timestampUnix":SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_secs(),"sourceCommit":option_env!("EFFECTCRAFT_DIAGNOSTIC_COMMIT").unwrap_or("development"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"worker":worker,"scan":scan,"dependencyHints":dependencies,"requestedMatchName":options.match_name,"matchingModules":matching.iter().map(|p|p.path.clone()).collect::<Vec<_>>(),"executions":[],"commercialCompatibilityVerified":false,"limitations":["Discovery does not execute plugin code. Only explicit --plugin, --self-test or exact PiPL --execute-matching targets are executed.","DLL imports are limited-search hints, not proof of missing dependencies. Transitive and dynamically loaded DLLs require local loader evidence.","Separate processes and deadlines contain failures; this is not an OS security sandbox.","Diagnostic render uses a generated 64x64 frame and native defaults. It does not apply the S_Shake FFX, custom Mocha data or licensing overrides.","No matching PiPL does not prove absence: registration-only modules need an explicitly selected --plugin path."]});
    save(&options.output, &report)?;
    if let Some(path) = options.preset {
        let converter = std::env::var_os("EFFECTCRAFT_AEP_CONVERTER").map(PathBuf::from).unwrap_or_else(|| executable.with_file_name("aep-effectcraft.exe"));
        report["preset"] = preset_analysis(&path, converter).unwrap_or_else(|e| json!({"source":path,"error":e,"appliedToNativePlugin":false}));
        save(&options.output, &report)?;
    }
    for plugin in targets {
        println!("Testing {}", plugin.display());
        let metadata = read_bounded(&plugin, 256 * 1024 * 1024).and_then(|b| crate::pe::inspect(&b));
        let is_fixture = fixture.as_ref() == Some(&plugin);
        let result = if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Err("native execution requires Windows x64".into())
        } else if let Err(e) = &metadata {
            Err(e.clone())
        } else {
            execute_test(&worker, &plugin, 0.0, if is_fixture { &[(1, 0.5)] } else { &[] })
        };
        let mut execution = result.unwrap_or_else(|e| json!({"plugin":plugin,"error":e,"presetApplied":false}));
        execution["registrationMatchesRequested"] = options
            .match_name
            .as_ref()
            .map(|name| json!(execution["checkpoint"]["identity"]["matchName"].as_str() == Some(name.as_str())))
            .unwrap_or(Value::Null);
        if let Ok(m) = &metadata {
            execution["dependencyHints"] =
                json!({"directImports":dependency_hints(&plugin,&m.imports),"delayImports":dependency_hints(&plugin,&m.delay_imports)});
        }
        execution["passiveMetadata"] = metadata.map(|m| json!(m)).unwrap_or_else(|e| json!({"error":e}));
        if is_fixture && execution["frame"]["verified"] == true {
            let second = execute_test(&worker, &plugin, 0.25, &[(1, 0.5)]).unwrap_or_else(|e| json!({"error":e}));
            let passed = execution["checkpoint"]["identity"]["matchName"] == "org.effectcraft.test.gain"
                && second["checkpoint"]["identity"]["matchName"] == "org.effectcraft.test.gain"
                && execution["frame"]["pixelsChanged"] == true
                && second["frame"]["verified"] == true
                && second["frame"]["pixelsChanged"] == true
                && execution["frame"]["gainFixtureMaximumError"].as_f64().is_some_and(|e| e < 1e-6)
                && second["frame"]["gainFixtureMaximumError"].as_f64().is_some_and(|e| e < 1e-6);
            execution["secondFrame"] = second;
            execution["selfTestPassed"] = json!(passed);
        }
        if let Some(executions) = report["executions"].as_array_mut() {
            executions.push(execution);
        }
        save(&options.output, &report)?;
    }
    println!("Diagnostic saved: {}", options.output.display());
    Ok(())
}
