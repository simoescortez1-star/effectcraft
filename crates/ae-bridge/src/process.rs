//! External `aep-effectcraft INPUT OUTPUT.ecproj` contract, never a shell command.
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::{Conversion, MAX_PROJECT_BYTES, MAX_REPORT_BYTES, MAX_SOURCE_BYTES, ProjectConverter};

/// The caller explicitly chooses a trusted executable. This is process isolation, not an OS sandbox.
pub struct ExternalConverter {
    pub executable: PathBuf,
    pub timeout: Duration,
}

struct Scratch(PathBuf);
impl Scratch {
    fn create() -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("effectcraft-aep-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("cannot create AEP conversion directory: {e}")),
            }
        }
        Err("cannot reserve an AEP conversion directory".into())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take((limit as u64).saturating_add(1)).read_to_end(&mut bytes).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!("{} exceeds {limit} bytes", path.display()));
    }
    Ok(bytes)
}

impl ProjectConverter for ExternalConverter {
    fn convert(&self, source: &[u8]) -> Result<Conversion, String> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err("AEP source exceeds 256 MiB".into());
        }
        if !self.executable.is_absolute() || !self.executable.is_file() {
            return Err("EFFECTCRAFT_AEP_CONVERTER must name an existing absolute executable path".into());
        }
        if self.timeout.is_zero() || self.timeout > Duration::from_secs(300) {
            return Err("AEP conversion timeout must be in (0, 300] seconds".into());
        }
        let scratch = Scratch::create()?;
        let input = scratch.0.join("source.aep");
        let output = scratch.0.join("converted.ecproj");
        let report = scratch.0.join("converted.ecproj.import.json");
        let log_path = scratch.0.join("converter.log");
        let log = File::create(&log_path).map_err(|e| format!("cannot create converter log: {e}"))?;
        // A converter receives only a task-owned snapshot, never the user's original path.
        fs::write(&input, source).map_err(|e| format!("cannot snapshot AEP: {e}"))?;
        let child = Command::new(&self.executable)
            .arg(&input)
            .arg(&output)
            .current_dir(&scratch.0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| format!("cannot launch AEP converter: {e}"))?;
        let mut worker = Worker(child);
        let started = Instant::now();
        loop {
            if let Some(status) = worker.0.try_wait().map_err(|e| format!("cannot poll AEP converter: {e}"))? {
                if !status.success() {
                    let log = read_bounded(&log_path, 1024 * 1024).unwrap_or_default();
                    let message: String = String::from_utf8_lossy(&log).chars().take(8192).collect();
                    return Err(format!("AEP converter failed ({status}); no project imported: {message}"));
                }
                break;
            }
            if started.elapsed() >= self.timeout {
                return Err("AEP converter timed out; worker terminated and project unchanged".into());
            }
            for (path, limit) in [(&output, MAX_PROJECT_BYTES), (&report, MAX_REPORT_BYTES), (&log_path, 1024 * 1024)] {
                if fs::metadata(path).is_ok_and(|m| m.len() > limit as u64) {
                    return Err("AEP converter exceeded its output size limit".into());
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let project_json = String::from_utf8(read_bounded(&output, MAX_PROJECT_BYTES)?).map_err(|_| "AEP converter project is not UTF-8")?;
        let report_json = String::from_utf8(read_bounded(&report, MAX_REPORT_BYTES)?).map_err(|_| "AEP converter report is not UTF-8")?;
        crate::warnings(&report_json)?;
        Ok(Conversion { project_json, report_json })
    }
}
