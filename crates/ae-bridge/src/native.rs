//! Explicit native loading in a disposable worker. No native ABI enters the editor.
use crate::process::{Scratch, Worker, read_bounded};
use effectcraft_effects::plugin::{EffectPlugin, PluginFrame, PluginManifest, PluginParam, PluginParamKind, PluginParams};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const MAX_PIXELS: usize = 4_194_304;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameter {
    index: usize,
    name: String,
    default: f64,
    min: f64,
    max: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Response {
    protocol: u32,
    loaded: bool,
    initialized: bool,
    parameters: Vec<Parameter>,
    rendered: bool,
    depth: u32,
    fidelity_verified: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeStatus {
    pub rendered: bool,
    pub diagnostic: String,
}

pub struct NativeEffect {
    executable: PathBuf,
    path: PathBuf,
    modified: SystemTime,
    bytes: u64,
    parameters: Vec<Parameter>,
    manifest: PluginManifest,
    status: Mutex<NativeStatus>,
    enabled: AtomicBool,
}

fn execute(
    executable: &Path,
    plugin: &Path,
    size: (u32, u32),
    pixels: Option<&[[f32; 4]]>,
    time: f64,
    params: &[(usize, f64)],
) -> Result<(Response, Vec<[f32; 4]>), String> {
    if !executable.is_absolute() || !executable.is_file() {
        return Err("native worker must be an existing absolute executable path".into());
    }
    let (width, height) = size;
    let count = u64::from(width).checked_mul(u64::from(height)).and_then(|n| usize::try_from(n).ok()).ok_or("native frame size overflow")?;
    if count > MAX_PIXELS || width > 8192 || height > 8192 || !time.is_finite() || time.abs() > 2_000_000.0 || params.len() > 256 {
        return Err("native frame exceeds geometry, time or parameter limits".into());
    }
    if pixels.is_some_and(|p| p.len() != count || p.iter().flatten().any(|v| !v.is_finite())) {
        return Err("invalid native input pixels".into());
    }
    let scratch = Scratch::create()?;
    if let Some(pixels) = pixels {
        let mut data = Vec::with_capacity(count.saturating_mul(16));
        for pixel in pixels {
            for value in pixel {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        fs::write(scratch.0.join("input.rgba32f"), data).map_err(|e| format!("cannot write native input: {e}"))?;
    }
    let response_path = scratch.0.join("response.json");
    let output_path = scratch.0.join("output.rgba32f");
    let request_path = scratch.0.join("request.json");
    let request =
        json!({"protocol":1,"operation":if pixels.is_some(){"render"}else{"probe"},"plugin":plugin,"width":width,"height":height,"time":time,"params":params});
    fs::write(&request_path, serde_json::to_vec(&request).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let log_path = scratch.0.join("worker.log");
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut command = Command::new(executable);
    command.args([&request_path, &response_path]).current_dir(&scratch.0).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(log));
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = Worker(command.spawn().map_err(|e| format!("cannot launch native worker: {e}"))?);
    let started = Instant::now();
    loop {
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                let log = read_bounded(&log_path, 1024 * 1024).unwrap_or_default();
                let message: String = String::from_utf8_lossy(&log).chars().take(4096).collect();
                return Err(format!("native worker failed ({status}): {message}"));
            }
            break;
        }
        if started.elapsed() >= Duration::from_secs(10) {
            return Err("native worker timed out after 10 seconds and was terminated".into());
        }
        for (path, limit) in [(&response_path, 1024 * 1024), (&log_path, 1024 * 1024), (&output_path, count.saturating_mul(16))] {
            if fs::metadata(path).is_ok_and(|m| m.len() > limit as u64) {
                return Err("native worker exceeded output size limit".into());
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let response: Response = serde_json::from_slice(&read_bounded(&response_path, 1024 * 1024)?).map_err(|e| format!("invalid native worker response: {e}"))?;
    if response.protocol != 1
        || !response.loaded
        || !response.initialized
        || response.depth != 32
        || response.fidelity_verified
        || response.rendered != pixels.is_some()
        || response.parameters.len() > 256
    {
        return Err("invalid native worker stage contract".into());
    }
    let mut indices = std::collections::BTreeSet::new();
    for parameter in &response.parameters {
        if parameter.index == 0
            || parameter.index > 256
            || !indices.insert(parameter.index)
            || parameter.name.len() > 1024
            || !parameter.default.is_finite()
            || !parameter.min.is_finite()
            || !parameter.max.is_finite()
            || parameter.min > parameter.default
            || parameter.default > parameter.max
        {
            return Err("invalid native parameter declaration".into());
        }
    }
    let mut output = Vec::new();
    if pixels.is_some() {
        let data = read_bounded(&output_path, count.saturating_mul(16))?;
        if data.len() != count.saturating_mul(16) {
            return Err("native output frame length mismatch".into());
        }
        output.reserve(count);
        for pixel in data.chunks_exact(16) {
            let mut rgba = [0.0; 4];
            for (value, channel) in rgba.iter_mut().zip(pixel.chunks_exact(4)) {
                *value = f32::from_le_bytes(channel.try_into().map_err(|_| "invalid native pixel channel")?);
                if !value.is_finite() {
                    return Err("non-finite native output".into());
                }
            }
            output.push(rgba);
        }
    }
    Ok((response, output))
}

impl NativeEffect {
    pub fn load(executable: PathBuf, path: PathBuf) -> Result<Arc<Self>, String> {
        if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            return Err("native AEX execution requires Windows x64".into());
        }
        let path = fs::canonicalize(path).map_err(|e| format!("cannot resolve native plugin: {e}"))?;
        let data = read_bounded(&path, 256 * 1024 * 1024)?;
        crate::discovery::inspect_pe(&data)?;
        let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hash);
        data.hash(&mut hash);
        let (response, _) = execute(&executable, &path, (0, 0), None, 0.0, &[])?;
        let manifest = PluginManifest {
            api: 1,
            id: format!("org.effectcraft.aex.{:016x}", hash.finish()),
            name: path.file_stem().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "AEX plugin".into()),
            category: "Native AEX (experimental)".into(),
            version: "experimental-1".into(),
            author: String::new(),
            description: "External CPU Smart Render, float sliders, 32-bit premultiplied RGBA. Visual fidelity unverified.".into(),
            params: response
                .parameters
                .iter()
                .map(|p| PluginParam {
                    id: format!("p{}", p.index),
                    name: p.name.clone(),
                    kind: PluginParamKind::Slider { default: p.default, min: p.min, max: p.max, slider_min: None, slider_max: None, decimals: 3 },
                })
                .collect(),
        };
        manifest.validate()?;
        Ok(Arc::new(Self {
            executable,
            path,
            modified: metadata.modified().map_err(|e| e.to_string())?,
            bytes: metadata.len(),
            parameters: response.parameters,
            manifest,
            status: Mutex::new(NativeStatus::default()),
            enabled: AtomicBool::new(true),
        }))
    }
    pub fn status(&self) -> NativeStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_else(|_| NativeStatus { rendered: false, diagnostic: "native status lock poisoned".into() })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}
impl EffectPlugin for NativeEffect {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
    fn source(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
    fn render(&self, frame: &mut PluginFrame, params: &PluginParams, time: f64) -> Result<(), String> {
        let result = (|| {
            if !self.enabled.load(Ordering::Relaxed) {
                return Err("native plugin disabled in Plugin Manager".into());
            }
            let metadata = fs::metadata(&self.path).map_err(|e| e.to_string())?;
            if metadata.len() != self.bytes || metadata.modified().map_err(|e| e.to_string())? != self.modified {
                return Err("native plugin changed since loading; reload it explicitly".into());
            }
            if params.flat.len() != self.parameters.len() {
                return Err("native parameter count mismatch".into());
            }
            let values: Vec<_> = self.parameters.iter().zip(params.flat).map(|(p, v)| (p.index, *v)).collect();
            let (response, output) = execute(&self.executable, &self.path, (frame.width, frame.height), Some(frame.pixels), time, &values)?;
            if response.parameters.iter().map(|p| p.index).ne(self.parameters.iter().map(|p| p.index)) {
                return Err("native parameter layout changed".into());
            }
            frame.pixels.copy_from_slice(&output);
            Ok(())
        })();
        if let Ok(mut status) = self.status.lock() {
            status.rendered = result.is_ok();
            status.diagnostic = result.as_ref().err().cloned().unwrap_or_else(|| "CPU Smart Render completed; visual fidelity unverified".into());
        }
        result
    }
}
