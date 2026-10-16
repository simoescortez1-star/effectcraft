//! Passive PE inspection. A valid DLL is a candidate, never a compatible effect.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginConfig {
    pub folders: Vec<String>,
    pub disabled: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Compatibility {
    Compatible,
    Partial,
    Unsupported,
    Untested,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub path: String,
    pub enabled: bool,
    pub compatibility: Compatibility,
    pub diagnostic: String,
    pub loaded: bool,
    pub initialized: bool,
    pub parameters: bool,
    pub rendered: bool,
    pub fidelity_verified: bool,
    #[serde(default)]
    pub metadata: Option<crate::pe::Metadata>,
}

impl Candidate {
    /// A header may permit an explicit load attempt, never automatic execution.
    pub fn can_load(&self) -> bool {
        self.enabled && self.compatibility != Compatibility::Unsupported
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Scan {
    pub plugins: Vec<Candidate>,
    pub diagnostics: Vec<String>,
}

/// PE/COFF header subset documented by Microsoft, checked without mapping/executing code.
pub fn inspect_pe(bytes: &[u8]) -> Result<(), String> {
    let pe = pe_offset(bytes)?;
    let end = pe.checked_add(26).ok_or("PE header offset overflow")?;
    inspect_header(bytes.get(pe..end).ok_or("truncated PE header")?)
}

fn pe_offset(bytes: &[u8]) -> Result<usize, String> {
    if bytes.get(..2) != Some(b"MZ") {
        return Err("not a Windows PE image (missing MZ)".into());
    }
    let raw: [u8; 4] = bytes.get(0x3c..0x40).and_then(|b| b.try_into().ok()).ok_or("truncated DOS header")?;
    let pe = usize::try_from(u32::from_le_bytes(raw)).map_err(|_| "PE offset exceeds address space")?;
    if pe > 1024 * 1024 {
        return Err("PE header offset exceeds inspection limit".into());
    }
    Ok(pe)
}

fn inspect_header(h: &[u8]) -> Result<(), String> {
    if h.get(..4) != Some(b"PE\0\0") || h.get(4..6) != Some(&[0x64, 0x86]) {
        return Err("not an AMD64 PE image".into());
    }
    let characteristics: [u8; 2] = h.get(22..24).and_then(|b| b.try_into().ok()).ok_or("missing PE characteristics")?;
    let optional_len: [u8; 2] = h.get(20..22).and_then(|b| b.try_into().ok()).ok_or("missing PE optional-header length")?;
    if u16::from_le_bytes(characteristics) & 0x2000 == 0 || u16::from_le_bytes(optional_len) < 2 || h.get(24..26) != Some(&[0x0b, 0x02]) {
        return Err("not a PE32+ DLL".into());
    }
    Ok(())
}

/// Never follows symlinks; bounds directory visits, files and depth, including non-plugin files.
#[cfg(not(target_arch = "wasm32"))]
pub fn scan(config: &PluginConfig) -> Scan {
    use std::collections::BTreeSet;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::PathBuf;
    let mut result = Scan::default();
    if config.folders.len() > 64 {
        result.diagnostics.push("At most 64 plugin folders may be scanned".into());
        return result;
    }
    let mut stack: Vec<_> = config.folders.iter().map(|p| (PathBuf::from(p), 0u8)).collect();
    let mut seen = BTreeSet::new();
    let mut visited = 0usize;
    let mut metadata_budget = 1024 * 1024 * 1024u64;
    while let Some((dir, depth)) = stack.pop() {
        if !dir.is_absolute() {
            result.diagnostics.push(format!("{}: folder path must be absolute", dir.display()));
            continue;
        }
        if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
            result.diagnostics.push(format!("{}: symlink skipped", dir.display()));
            continue;
        }
        let canonical = match std::fs::canonicalize(&dir) {
            Ok(p) => p,
            Err(e) => {
                result.diagnostics.push(format!("{}: {e}", dir.display()));
                continue;
            }
        };
        if !seen.insert(canonical.clone()) {
            continue;
        }
        let entries = match std::fs::read_dir(&canonical) {
            Ok(e) => e,
            Err(e) => {
                result.diagnostics.push(format!("{}: {e}", dir.display()));
                continue;
            }
        };
        for entry in entries {
            visited += 1;
            if visited > 20_000 {
                result.diagnostics.push("Scan stopped at 20000 directory entries".into());
                return result;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    result.diagnostics.push(e.to_string());
                    continue;
                }
            };
            let kind = match entry.file_type() {
                Ok(t) => t,
                Err(e) => {
                    result.diagnostics.push(e.to_string());
                    continue;
                }
            };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if depth < 16 {
                    stack.push((path, depth + 1));
                } else {
                    result.diagnostics.push(format!("{}: depth limit reached", path.display()));
                }
                continue;
            }
            if !kind.is_file() || !path.extension().is_some_and(|x| x.eq_ignore_ascii_case("aex")) {
                continue;
            }
            if result.plugins.len() >= 4096 {
                result.diagnostics.push("Scan stopped at 4096 plugin candidates".into());
                return result;
            }
            let checked = (|| -> Result<(), String> {
                let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                let mut dos = [0; 64];
                file.read_exact(&mut dos).map_err(|e| format!("cannot read DOS header: {e}"))?;
                let offset = pe_offset(&dos)?;
                file.seek(SeekFrom::Start(offset as u64)).map_err(|e| e.to_string())?;
                let mut header = [0; 26];
                file.read_exact(&mut header).map_err(|e| format!("cannot read PE header: {e}"))?;
                inspect_header(&header)
            })();
            let mut metadata = None;
            if checked.is_ok() {
                match std::fs::metadata(&path) {
                    Ok(m) if m.len() <= metadata_budget && m.len() <= 256 * 1024 * 1024 => {
                        metadata_budget = metadata_budget.saturating_sub(m.len());
                        let inspected = (|| -> Result<crate::pe::Metadata, String> {
                            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                            let mut data = Vec::new();
                            file.take(256 * 1024 * 1024 + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
                            if data.len() > 256 * 1024 * 1024 {
                                return Err("plugin changed or exceeds metadata inspection limit".into());
                            }
                            crate::pe::inspect(&data)
                        })();
                        match inspected {
                            Ok(m) => metadata = Some(m),
                            Err(e) => result.diagnostics.push(format!("{}: metadata: {e}", path.display())),
                        }
                    }
                    _ => result.diagnostics.push(format!("{}: metadata skipped (256 MiB file / 1 GiB scan read budget)", path.display())),
                }
            }
            let path = path.to_string_lossy().to_string();
            let (mut compatibility, mut diagnostic) = match checked {
                Ok(()) => (
                    Compatibility::Untested,
                    "AMD64 PE32+ DLL candidate only. Scanning never executes code; use Load and test with the optional Windows worker.".into(),
                ),
                Err(e) => (Compatibility::Unsupported, e),
            };
            if metadata.as_ref().is_some_and(|m| !m.effects.is_empty() && m.effects.iter().all(|e| e.kind != "eFKT")) {
                compatibility = Compatibility::Unsupported;
                diagnostic = "PiPL metadata identifies non-effect extensions only; no effect runtime will be attempted.".into();
            }
            result.plugins.push(Candidate {
                enabled: !config.disabled.contains(&path),
                path,
                compatibility,
                diagnostic,
                loaded: false,
                initialized: false,
                parameters: false,
                rendered: false,
                fidelity_verified: false,
                metadata,
            });
        }
    }
    result.plugins.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

#[cfg(target_arch = "wasm32")]
pub fn scan(_: &PluginConfig) -> Scan {
    Scan { diagnostics: vec!["Local native-plugin discovery is unavailable in the browser".into()], ..Scan::default() }
}

/// Conventional Windows install locations; enumeration itself remains an explicit command.
pub fn default_folders() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let mut folders = vec![];
        if let Some(root) = std::env::var_os("ProgramFiles") {
            let root = std::path::PathBuf::from(root);
            folders.push(root.join("Adobe/Common/Plug-ins/7.0/MediaCore").to_string_lossy().to_string());
            let adobe = root.join("Adobe");
            if let Ok(entries) = std::fs::read_dir(adobe) {
                for e in entries.take(256).flatten() {
                    if e.file_name().to_string_lossy().starts_with("Adobe After Effects") {
                        let path = e.path().join("Support Files/Plug-ins");
                        if path.is_dir() {
                            folders.push(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        folders
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![]
    }
}
