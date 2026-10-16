//! Explicit native-plugin execution and data-only external preset conversion.
use super::{CommandSpec, always, bad, has_layers, layer_mut, layers_p, str_p};
use crate::{Result, Session, cmd};
use effectcraft_ae_bridge::discovery;
use effectcraft_project::{ItemKind, Node, Project};
use serde_json::{Value, json};

fn change_folders(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "folder").ok_or_else(|| bad("effect.aex.addFolder", "missing folder"))?;
    if path.len() > 4096 || !std::path::Path::new(path).is_absolute() {
        return Err(bad("effect.aex.addFolder", "folder must be an absolute path of at most 4096 bytes"));
    }
    let remove = p.get("remove").map(|v| v.as_bool().ok_or_else(|| bad("effect.aex.addFolder", "remove must be a boolean"))).transpose()?.unwrap_or(false);
    let config = &mut s.prefs.after_effects;
    if remove {
        config.folders.retain(|f| f != path);
    } else if !config.folders.iter().any(|f| f == path) {
        if config.folders.len() >= 64 {
            return Err(bad("effect.aex.addFolder", "64 folder limit reached"));
        }
        config.folders.push(path.into());
    }
    s.save_prefs();
    s.ae_plugins = Default::default();
    Ok(json!({"folders": s.prefs.after_effects.folders}))
}

fn scan(s: &mut Session, p: &Value) -> Result<Value> {
    let installed = p.get("installed").map(|v| v.as_bool().ok_or_else(|| bad("effect.aex.scan", "installed must be a boolean"))).transpose()?.unwrap_or(false);
    let mut config = s.prefs.after_effects.clone();
    if installed {
        for folder in discovery::default_folders() {
            if !config.folders.contains(&folder) {
                config.folders.push(folder);
            }
        }
    }
    s.ae_plugins = discovery::scan(&config);
    refresh_native(s);
    Ok(json!(s.ae_plugins))
}

fn refresh_native(s: &mut Session) {
    #[cfg(not(target_arch = "wasm32"))]
    for candidate in &mut s.ae_plugins.plugins {
        for effect in s.ae_native_effects.values().filter(|effect| effect.path() == std::path::Path::new(&candidate.path)) {
            let status = effect.status();
            candidate.loaded = true;
            candidate.initialized = true;
            candidate.parameters = true;
            candidate.rendered = status.rendered;
            candidate.compatibility = discovery::Compatibility::Partial;
            candidate.diagnostic = status.diagnostic;
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = s;
}

fn toggle(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad("effect.aex.enable", "missing path"))?;
    let enabled = p.get("enabled").and_then(Value::as_bool).ok_or_else(|| bad("effect.aex.enable", "missing enabled boolean"))?;
    // Windows canonical paths use an extended prefix; clients may supply normal DOS paths.
    let canonical = std::fs::canonicalize(path).ok();
    let entry = s
        .ae_plugins
        .plugins
        .iter_mut()
        .find(|e| e.path == path || canonical.as_ref().is_some_and(|p| std::path::Path::new(&e.path) == p))
        .ok_or_else(|| bad("effect.aex.enable", "scan the plugin before changing its state"))?;
    let path = entry.path.clone();
    if !enabled && !s.prefs.after_effects.disabled.iter().any(|x| x == &path) && s.prefs.after_effects.disabled.len() >= 20_000 {
        return Err(bad("effect.aex.enable", "disabled plugin limit reached"));
    }
    entry.enabled = enabled;
    s.prefs.after_effects.disabled.retain(|x| x != &path);
    if !enabled {
        s.prefs.after_effects.disabled.push(path.clone());
    }
    #[cfg(not(target_arch = "wasm32"))]
    for effect in s.ae_native_effects.values().filter(|effect| effect.path() == std::path::Path::new(&path)) {
        effect.set_enabled(enabled);
    }
    let loaded = entry.loaded;
    s.save_prefs();
    if loaded {
        s.layer_cache.clear();
        s.events.push(crate::Event::PurgeCaches);
        s.bump();
    }
    Ok(json!({"path": path, "enabled": enabled, "loaded": loaded}))
}

fn native_available(_: &Session) -> std::result::Result<(), String> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) { Ok(()) } else { Err("native AEX execution requires Windows x64".into()) }
}

#[cfg(not(target_arch = "wasm32"))]
fn worker_executable() -> Option<std::path::PathBuf> {
    std::env::var_os("EFFECTCRAFT_AEX_WORKER")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok().map(|p| p.with_file_name("effectcraft-aex-worker.exe")))
}

fn runtime_info() -> Value {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let worker = worker_executable();
        let found = worker.as_ref().is_some_and(|p| p.is_absolute() && p.is_file());
        let folder = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join("test-plugins"))).filter(|p| p.join("EffectCraft Gain.aex").is_file());
        json!({"platformSupported":cfg!(all(target_os="windows",target_arch="x86_64")),"workerFound":found,"worker":worker,"bundledTestFolder":folder})
    }
    #[cfg(target_arch = "wasm32")]
    json!({"platformSupported":false,"workerFound":false,"worker":null,"bundledTestFolder":null})
}

fn load_native(s: &mut Session, p: &Value) -> Result<Value> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (s, p);
        Err(bad("effect.aex.load", "native AEX execution is unavailable on the web"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use effectcraft_effects::plugin::{EffectPlugin, PluginFrame, PluginParams};
        const C: &str = "effect.aex.load";
        let path = str_p(p, "path").ok_or_else(|| bad(C, "missing plugin path"))?;
        let path = std::fs::canonicalize(path).map_err(|e| bad(C, format!("cannot resolve plugin: {e}")))?;
        let candidate = s
            .ae_plugins
            .plugins
            .iter()
            .find(|entry| std::path::Path::new(&entry.path) == path)
            .ok_or_else(|| bad(C, "scan this plugin folder before explicitly loading it"))?;
        if !candidate.enabled || candidate.compatibility == discovery::Compatibility::Unsupported {
            return Err(bad(C, "plugin is disabled or its PE header is unsupported"));
        }
        let executable = worker_executable().ok_or_else(|| bad(C, "cannot locate native worker"))?;
        let effect = match effectcraft_ae_bridge::native::NativeEffect::load(executable, path.clone()) {
            Ok(effect) => effect,
            Err(error) => {
                if let Some(candidate) = s.ae_plugins.plugins.iter_mut().find(|entry| std::path::Path::new(&entry.path) == path) {
                    candidate.compatibility = discovery::Compatibility::Partial;
                    candidate.diagnostic = format!("Explicit worker load failed; initialization/parameter stages unconfirmed: {error}");
                }
                return Err(bad(C, error));
            }
        };
        let id = effect.manifest().id.clone();
        let registered = s.ae_native_effects.contains_key(&id);
        let effect = s.ae_native_effects.get(&id).cloned().unwrap_or(effect);
        effect.set_enabled(true);
        // One explicit load also tests a tiny real frame. This proves execution,
        // never visual equivalence or compatibility with other plugins.
        let defaults: Vec<_> = effect
            .manifest()
            .params
            .iter()
            .filter_map(|p| match p.kind {
                effectcraft_effects::plugin::PluginParamKind::Slider { default, .. } => Some(default),
                _ => None,
            })
            .collect();
        let mut pixels = [[0.1, 0.2, 0.3, 1.0]; 4];
        let rendered = effect.render(
            &mut PluginFrame { width: 2, height: 2, pixels: &mut pixels, scale: 1.0 },
            &PluginParams { manifest: effect.manifest(), flat: &defaults },
            0.0,
        );
        if let Some(candidate) = s.ae_plugins.plugins.iter_mut().find(|entry| std::path::Path::new(&entry.path) == path) {
            candidate.loaded = true;
            candidate.initialized = true;
            candidate.parameters = true;
            candidate.rendered = rendered.is_ok();
            candidate.compatibility = discovery::Compatibility::Partial;
            candidate.diagnostic = effect.status().diagnostic;
        }
        rendered.map_err(|e| bad(C, e))?;
        if !registered {
            effectcraft_effects::plugin::register_plugin(effect.clone()).map_err(|e| bad(C, e))?;
            s.ae_native_effects.insert(id.clone(), effect);
        }
        s.layer_cache.clear();
        s.events.push(crate::Event::PurgeCaches);
        s.bump();
        Ok(json!({"effect":id,"loaded":true,"initialized":true,"parameters":defaults.len(),"rendered":true,"fidelityVerified":false,"compatibility":"partial"}))
    }
}

fn load_and_apply_native(s: &mut Session, p: &Value) -> Result<Value> {
    let (_, layers) = layers_p(s, p)?;
    if layers.is_empty() {
        return Err(bad("effect.aex.loadAndApply", "select a layer before loading and applying a plugin"));
    }
    let mut loaded = load_native(s, p)?;
    let effect = loaded.get("effect").and_then(Value::as_str).ok_or_else(|| bad("effect.aex.loadAndApply", "native load returned no registered effect"))?;
    let applied = s.execute("effect.apply", json!({"effect":effect,"layers":layers.iter().map(|id|id.0).collect::<Vec<_>>()}))?;
    loaded["applied"] = applied;
    Ok(loaded)
}

/// Apply only explicitly mapped properties from the data-only external preset carrier.
/// Unavailable effects retain their original bytes and tree in the layer comment.
fn apply_preset(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "anim.applyAfterEffectsPreset";
    const PRESERVATION_HEADER: &str = "\nEffectCraft FFX preservation:\n";
    let path = str_p(p, "path").filter(|p| p.to_ascii_lowercase().ends_with(".ffx")).ok_or_else(|| bad(C, "expected an .ffx preset"))?;
    let (cid, ids) = layers_p(s, p)?;
    if ids.is_empty() || ids.len() > 16 {
        return Err(bad(C, "select between 1 and 16 target layers"));
    }
    let converter = s.ae_converter.as_ref().ok_or_else(|| bad(C, "FFX requires the separate AEP/FFX converter supplied with this edition"))?;
    let bytes = s.services.read_file_bounded(path, 1024 * 1024).map_err(|e| bad(C, format!("cannot read preset: {e}")))?;
    let converted = converter.convert(&bytes).map_err(|e| bad(C, e))?;
    if converted.project_json.len() > effectcraft_ae_bridge::MAX_PROJECT_BYTES {
        return Err(bad(C, "preset carrier exceeds 128 MiB"));
    }
    let warnings = effectcraft_ae_bridge::warnings(&converted.report_json).map_err(|e| bad(C, e))?;
    let carrier = Project::from_json(&converted.project_json).map_err(|e| bad(C, format!("invalid preset carrier: {e}")))?;
    let layers: Vec<_> = carrier
        .items
        .values()
        .filter_map(|i| match &i.kind {
            ItemKind::Comp(c) => Some(&c.layers),
            _ => None,
        })
        .flatten()
        .collect();
    let [source] = layers.as_slice() else {
        return Err(bad(C, "preset carrier must contain exactly one layer"));
    };
    if source.comment.len() > 6 * 1024 * 1024 {
        return Err(bad(C, "preset metadata exceeds 6 MiB"));
    }
    let metadata: Value = serde_json::from_str(&source.comment).map_err(|e| bad(C, format!("invalid preset metadata: {e}")))?;
    if metadata.get("effectcraftPreset").and_then(Value::as_u64) != Some(1) {
        return Err(bad(C, "converter has no supported FFX carrier contract"));
    }
    let paths = metadata.get("mappedPaths").and_then(Value::as_array).ok_or_else(|| bad(C, "missing preset property map"))?;
    if paths.len() > 32 {
        return Err(bad(C, "preset property map exceeds 32 paths"));
    }
    let mut properties = Vec::new();
    for path in paths {
        let path = path.as_str().filter(|p| p.starts_with("transform/") && p.len() < 128).ok_or_else(|| bad(C, "invalid preset property path"))?;
        let property = source.props.prop(path).ok_or_else(|| bad(C, "preset property missing from carrier"))?;
        properties.push((path.to_string(), property.clone()));
    }
    let mut effects = Vec::new();
    if let Some(group) = source.props.sub("effects") {
        for effect in group.groups() {
            if effectcraft_effects::lookup(&effect.match_id).is_none() {
                return Err(bad(C, "preset contains an unregistered effect"));
            }
            effects.push(effect.clone());
        }
    }
    let unavailable = metadata.get("unavailableEffects").cloned().unwrap_or_else(|| json!([]));
    let mut uid_count = effects.len() as u64;
    let mut pending: Vec<_> = effects.iter().collect();
    while let Some(group) = pending.pop() {
        uid_count = uid_count.checked_add(group.children.len() as u64).ok_or_else(|| bad(C, "preset node count overflow"))?;
        pending.extend(group.groups());
    }
    let required = uid_count.checked_mul(ids.len() as u64).ok_or_else(|| bad(C, "preset UID count overflow"))?;
    if s.project.next_id.checked_add(required).is_none() {
        return Err(bad(C, "not enough IDs for preset effects"));
    }
    let applied = !properties.is_empty() || !effects.is_empty();
    let property_count = properties.len().saturating_mul(ids.len());
    let keyframe_count = properties.iter().fold(0_usize, |total, (_, p)| total.saturating_add(p.keys.len())).saturating_mul(ids.len());
    let mut revealed = Vec::new();
    let now = s.time();
    let report: Value = serde_json::from_str(&converted.report_json).map_err(|e| bad(C, e.to_string()))?;
    let record = json!({"source":path,"preset":metadata,"conversionReport":report}).to_string();
    if record.len().checked_mul(ids.len()).is_none_or(|n| n > 64 * 1024 * 1024) {
        return Err(bad(C, "preset preservation exceeds 64 MiB"));
    }
    s.edit("Apply After Effects Preset", None, |project, _| {
        for id in &ids {
            let mut next = project.next_id;
            let layer = layer_mut(project, cid, *id)?;
            let at = layer.layer_time(now);
            for (path, source) in &properties {
                let target = layer.props.prop_mut(path).ok_or_else(|| bad(C, format!("target layer has no {path}")))?;
                if std::mem::discriminant(&source.value) != std::mem::discriminant(&target.value) {
                    return Err(bad(C, "preset property dimensions differ"));
                }
                let mut keys = source.keys.clone();
                for key in &mut keys {
                    key.time = effectcraft_time::Tick(key.time.0.checked_add(at.0).ok_or_else(|| bad(C, "preset key time overflow"))?);
                }
                target.value = source.value.clone();
                target.keys = keys;
                revealed.push(json!({"layer":id.0,"prop":target.uid}));
                target.expr = source.expr.clone();
                if let Some(expression) = &mut target.expr {
                    expression.enabled = false;
                }
            }
            if !effects.is_empty() {
                let group = layer.props.sub_mut("effects").ok_or_else(|| bad(C, "target layer has no effects"))?;
                for source in &effects {
                    let mut effect = source.clone();
                    let mut overflow = false;
                    effect.walk_mut(&mut |property| {
                        for key in &mut property.keys {
                            if let Some(time) = key.time.0.checked_add(at.0) {
                                key.time = effectcraft_time::Tick(time);
                            } else {
                                overflow = true;
                            }
                        }
                        if let Some(expression) = &mut property.expr {
                            expression.enabled = false;
                        }
                    });
                    if overflow {
                        return Err(bad(C, "preset effect key time overflow"));
                    }
                    effect.reassign_uids(&mut next);
                    group.children.push(Node::Group(effect));
                }
            }
            if layer.comment.len().checked_add(record.len()).and_then(|n| n.checked_add(PRESERVATION_HEADER.len())).is_none_or(|n| n > 8 * 1024 * 1024) {
                return Err(bad(C, "layer preset history exceeds 8 MiB"));
            }
            layer.comment.push_str(PRESERVATION_HEADER);
            layer.comment.push_str(&record);
            project.next_id = next;
        }
        Ok(())
    })?;
    if !revealed.is_empty() {
        s.events.push(crate::Event::Frontend {
            command: "timeline.revealProps".into(),
            params: json!({"props":revealed,"layers":ids.iter().map(|id|id.0).collect::<Vec<_>>()}),
        });
    }
    let summary = if applied {
        format!(
            "Applied {property_count} mapped transform property/properties and {keyframe_count} transform keyframe(s). Added {} effect(s). Inspect the details for timing and fidelity limits.",
            effects.len().saturating_mul(ids.len())
        )
    } else if unavailable.as_array().is_some_and(|effects| !effects.is_empty()) {
        "Preset preserved as data only. Required effects are unavailable; rendered pixels were not changed.".to_string()
    } else {
        "Preset preserved as data only. No supported property/effect mapping was produced; rendered pixels were not changed. Inspect the property diagnostics."
            .to_string()
    };
    s.events.push(crate::Event::Frontend {
        command: "app.showReport".into(),
        params: json!({"title":"After Effects Preset Report","text":warnings.join("\n"),"summary":format!("{summary}\n{}",effectcraft_ae_bridge::warning_summary(&warnings))}),
    });
    Ok(
        json!({"applied":applied,"preserved":true,"layers":ids.iter().map(|id|id.0).collect::<Vec<_>>(),"propertiesApplied":property_count,"keyframesApplied":keyframe_count,"effectsApplied":effects.len().saturating_mul(ids.len()),"mappedPaths":paths,"sourceTicksPerSecond":metadata.get("sourceTicksPerSecond"),"unavailableEffects":unavailable,"warnings":warnings,"timingVerified":false}),
    )
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("effect.aex.manager", "Plugin Manager...", ["Effect"], None, "{} — open passive AEX plugin manager", always, |s, _| {
            s.events.push(crate::Event::Frontend { command: "effect.aex.manager".into(), params: json!({}) });
            Ok(Value::Null)
        }),
        cmd!("effect.aex.addFolder", "Add Plugin Folder...", [], None, "{folder, remove? (bool)} — persist scan directories", always, change_folders),
        cmd!("effect.aex.scan", "Scan Native Plugins", [], None, "{installed? (bool)} — inspect files without executing DLLs", always, scan),
        cmd!("effect.aex.list", "List Native Plugins", [], None, "{} — candidates, runtime stage results and diagnostics", always, |s, _| {
            refresh_native(s);
            Ok(json!({"folders": s.prefs.after_effects.folders, "scan": s.ae_plugins, "runtime": runtime_info()}))
        }),
        cmd!("effect.aex.enable", "Enable Native Plugin", [], None, "{path, enabled (bool)} — availability preference only, never loads code", always, toggle),
        cmd!(
            "effect.aex.load",
            "Load and Test Native Plugin",
            [],
            None,
            "{path} — explicitly execute a scanned plugin in the external Windows worker; CPU Smart Render and float sliders only",
            native_available,
            load_native
        ),
        cmd!(
            "effect.aex.loadAndApply",
            "Load, Test and Apply Native Plugin",
            [],
            None,
            "{path,layers?} — explicitly load/test a scanned AEX, then add its registered effect to selected layers",
            |s| {
                native_available(s)?;
                has_layers(s)
            },
            load_and_apply_native
        ),
        cmd!(
            "anim.applyAfterEffectsPreset",
            "Apply After Effects Preset...",
            ["Animation"],
            None,
            "{path, layers?} — apply mapped FFX properties at current time; preserve unavailable effects as data with diagnostics",
            has_layers,
            apply_preset
        ),
    ]
}
