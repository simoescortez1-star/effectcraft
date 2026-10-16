//! Clean-room boundary: the editor does not link a third-party AEP decoder or native ABI.
//! Conversion is explicit and out of process. Discovery never loads a library.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod discovery;
#[cfg(not(target_arch = "wasm32"))]
pub mod process;

/// Maximum source size, including the snapshot passed to an external converter.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_PROJECT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_REPORT_BYTES: usize = 8 * 1024 * 1024;

/// Output is data only. The engine validates the project before editing a session.
#[derive(Debug)]
pub struct Conversion {
    pub project_json: String,
    pub report_json: String,
}

pub trait ProjectConverter: Send + Sync {
    fn convert(&self, source: &[u8]) -> Result<Conversion, String>;
}

/// Check the external report contract rather than silently losing converter diagnostics.
pub fn warnings(report: &str) -> Result<Vec<String>, String> {
    if report.len() > MAX_REPORT_BYTES {
        return Err("AEP conversion report exceeds 8 MiB".into());
    }
    let value: serde_json::Value = serde_json::from_str(report).map_err(|e| format!("invalid AEP conversion report: {e}"))?;
    let warnings =
        value.get("report").and_then(|r| r.get("warnings")).and_then(serde_json::Value::as_array).ok_or("AEP converter report has no warnings array")?;
    if warnings.len() > 10_000 {
        return Err("AEP converter report exceeds 10000 warnings".into());
    }
    warnings
        .iter()
        .map(|w| {
            let context = w.get("context").and_then(serde_json::Value::as_str).ok_or("AEP warning has no context")?;
            let message = w.get("message").and_then(serde_json::Value::as_str).ok_or("AEP warning has no message")?;
            Ok(format!("{context}: {message}"))
        })
        .collect()
}
