//! Bounded, passive metadata reader based on Microsoft's PE/COFF format and PiPL records.
//! An import search is only a hint: it does not model Windows' complete DLL loader.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EffectIdentity {
    pub source: String,
    pub name: String,
    pub match_name: String,
    pub category: String,
    pub kind: String,
    pub version_raw: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Metadata {
    pub architecture: String,
    pub file_version: Option<String>,
    pub imports: Vec<String>,
    pub delay_imports: Vec<String>,
    pub exports: Vec<String>,
    pub effects: Vec<EffectIdentity>,
    pub diagnostics: Vec<String>,
}

fn bytes(data: &[u8], at: usize, size: usize) -> Result<&[u8], String> {
    data.get(at..at.checked_add(size).ok_or("PE offset overflow")?).ok_or_else(|| "truncated PE metadata".into())
}
fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(bytes(data, at, 2)?.try_into().map_err(|_| "PE word")?))
}
fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(bytes(data, at, 4)?.try_into().map_err(|_| "PE dword")?))
}
fn cstring(data: &[u8], at: usize) -> Result<String, String> {
    let tail = data.get(at..).ok_or("PE string offset")?;
    let len = tail.iter().take(1024).position(|b| *b == 0).ok_or("unterminated PE string")?;
    String::from_utf8(bytes(tail, 0, len)?.to_vec()).map_err(|_| "non-UTF8 metadata string; not used for identity".into())
}

struct Image<'a> {
    data: &'a [u8],
    directories: usize,
    directory_count: usize,
    sections: Vec<(u32, u32, u32)>,
    headers: u32,
}
impl<'a> Image<'a> {
    fn new(data: &'a [u8]) -> Result<Self, String> {
        crate::discovery::inspect_pe(data)?;
        let pe = u32_at(data, 60)? as usize;
        let optional = pe.checked_add(24).ok_or("PE offset")?;
        let optional_size = u16_at(data, pe + 20)? as usize;
        if optional_size < 112 {
            return Err("PE32+ optional header lacks data directories".into());
        }
        bytes(data, optional, optional_size)?;
        let count = u16_at(data, pe + 6)? as usize;
        if count > 96 {
            return Err("PE section limit (96)".into());
        }
        let section_table = optional.checked_add(optional_size).ok_or("PE sections offset")?;
        let mut sections = Vec::new();
        for i in 0..count {
            let entry = section_table.checked_add(i * 40).ok_or("PE section overflow")?;
            bytes(data, entry, 40)?;
            sections.push((u32_at(data, entry + 12)?, u32_at(data, entry + 16)?, u32_at(data, entry + 20)?));
        }
        let directory_count = (u32_at(data, optional + 108)? as usize).min((optional_size - 112) / 8).min(16);
        Ok(Self { data, directories: optional + 112, directory_count, sections, headers: u32_at(data, optional + 60)? })
    }
    fn offset(&self, rva: u32, len: usize) -> Result<usize, String> {
        if rva < self.headers && (len as u64).saturating_add(u64::from(rva)) <= u64::from(self.headers) {
            bytes(self.data, rva as usize, len)?;
            return Ok(rva as usize);
        }
        for &(start, size, raw) in &self.sections {
            if let Some(delta) = rva.checked_sub(start)
                && u64::from(delta).saturating_add(len as u64) <= u64::from(size)
            {
                let at = raw.checked_add(delta).ok_or("PE file offset overflow")? as usize;
                bytes(self.data, at, len)?;
                return Ok(at);
            }
        }
        Err("PE RVA is outside initialized section data".into())
    }
    fn directory(&self, index: usize) -> Result<Option<(u32, u32)>, String> {
        if index >= self.directory_count {
            return Ok(None);
        }
        let at = self.directories + index * 8;
        let rva = u32_at(self.data, at)?;
        let size = u32_at(self.data, at + 4)?;
        Ok((rva != 0 && size != 0).then_some((rva, size)))
    }
    fn imports(&self, directory: usize) -> Result<Vec<String>, String> {
        let Some((rva, size)) = self.directory(directory)? else { return Ok(vec![]) };
        let stride = if directory == 13 { 32 } else { 20 };
        let count = (size as usize / stride).min(1025);
        let mut names = Vec::new();
        for i in 0..count {
            let entry_rva = rva.checked_add((i * stride) as u32).ok_or("PE imports overflow")?;
            let at = self.offset(entry_rva, stride)?;
            if bytes(self.data, at, stride)?.iter().all(|b| *b == 0) {
                names.sort();
                names.dedup();
                return Ok(names);
            }
            if i >= 1024 {
                return Err("PE import limit (1024)".into());
            }
            if directory == 13 && u32_at(self.data, at)? & 1 == 0 {
                return Err("VA-based delay imports are not decoded".into());
            }
            let name_rva = u32_at(self.data, at + if directory == 13 { 4 } else { 12 })?;
            let name = cstring(self.data, self.offset(name_rva, 1)?)?;
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
                return Err("invalid imported DLL basename".into());
            }
            names.push(name);
        }
        Err("unterminated PE import table".into())
    }
    fn exports(&self) -> Result<Vec<String>, String> {
        let Some((rva, _)) = self.directory(0)? else { return Ok(vec![]) };
        let at = self.offset(rva, 40)?;
        let count = u32_at(self.data, at + 24)? as usize;
        if count > 4096 {
            return Err("PE export name limit (4096)".into());
        }
        let table = self.offset(u32_at(self.data, at + 32)?, count * 4)?;
        (0..count).map(|i| cstring(self.data, self.offset(u32_at(self.data, table + i * 4)?, 1)?)).collect()
    }
    fn resources(&self, out: &mut Metadata) -> Result<(), String> {
        let Some((rva, size)) = self.directory(2)? else { return Ok(()) };
        if size > 32 * 1024 * 1024 {
            return Err("PE resources exceed 32 MiB".into());
        }
        let root = self.offset(rva, size as usize)?;
        let tree = bytes(self.data, root, size as usize)?;
        let mut pending = vec![(0usize, 0u8, String::new())];
        let mut visited = std::collections::BTreeSet::new();
        let mut entries = 0usize;
        while let Some((at, depth, inherited)) = pending.pop() {
            if !visited.insert(at) {
                return Err("cyclic or aliased PE resource tree".into());
            }
            let count = usize::from(u16_at(tree, at + 12)?).saturating_add(usize::from(u16_at(tree, at + 14)?));
            entries = entries.saturating_add(count);
            if entries > 4096 {
                return Err("PE resource entry limit (4096)".into());
            }
            for i in 0..count {
                let entry = at.checked_add(16 + i * 8).ok_or("resource offset overflow")?;
                let name = u32_at(tree, entry)?;
                let target = u32_at(tree, entry + 4)?;
                let kind = if depth == 0 {
                    if name & 0x8000_0000 == 0 {
                        name.to_string()
                    } else {
                        let at = (name & 0x7fff_ffff) as usize;
                        let len = usize::from(u16_at(tree, at)?);
                        if len > 128 {
                            return Err("resource type name too long".into());
                        }
                        let units: Vec<_> = bytes(tree, at + 2, len * 2)?.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
                        String::from_utf16(&units).map_err(|_| "invalid UTF16 resource type")?
                    }
                } else {
                    inherited.clone()
                };
                let target_at = (target & 0x7fff_ffff) as usize;
                if target & 0x8000_0000 != 0 {
                    if depth >= 3 {
                        return Err("resource tree depth exceeds 3".into());
                    }
                    pending.push((target_at, depth + 1, kind));
                } else {
                    let value_rva = u32_at(tree, target_at)?;
                    let value_size = u32_at(tree, target_at + 4)? as usize;
                    if value_size > 4 * 1024 * 1024 {
                        return Err("resource value exceeds 4 MiB".into());
                    }
                    let value = bytes(self.data, self.offset(value_rva, value_size)?, value_size)?;
                    if kind == "PiPL" {
                        match pipl(value) {
                            Ok(effect) => {
                                if !out.effects.contains(&effect) {
                                    out.effects.push(effect);
                                }
                            }
                            Err(e) => out.diagnostics.push(format!("PiPL: {e}")),
                        }
                    } else if kind == "16" {
                        // VS_VERSIONINFO: fixed file information follows the UTF16 key, aligned to 4 bytes.
                        let key: Vec<_> = "VS_VERSION_INFO\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
                        if bytes(value, 6, key.len()).is_ok_and(|b| b == key) {
                            let at = (6 + key.len() + 3) & !3;
                            if u32_at(value, at)? == 0xfeef04bd {
                                let hi = u32_at(value, at + 8)?;
                                let lo = u32_at(value, at + 12)?;
                                out.file_version = Some(format!("{}.{}.{}.{}", hi >> 16, hi & 65535, lo >> 16, lo & 65535));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl PartialEq for EffectIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.match_name == other.match_name && self.name == other.name && self.kind == other.kind && self.version_raw == other.version_raw
    }
}

/// Decode a complete PiPL record, not arbitrary occurrences of a match-name string.
/// Accept canonical big-endian records and Windows little-endian records explicitly.
fn pipl(data: &[u8]) -> Result<EffectIdentity, String> {
    for little in [false, true] {
        let read = |at| -> Result<u32, String> {
            let b = bytes(data, at, 4)?.try_into().map_err(|_| "PiPL word")?;
            Ok(if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
        };
        if read(0)? != 0 {
            continue;
        }
        let count = read(4)? as usize;
        if count == 0 || count > 256 {
            continue;
        }
        let mut at = 8usize;
        let mut effect = EffectIdentity { source: "PE/PiPL".into(), ..Default::default() };
        for _ in 0..count {
            let vendor = read(at)?;
            let key = read(at + 4)?.to_be_bytes();
            let len = read(at + 12)? as usize;
            if vendor != u32::from_be_bytes(*b"8BIM") || len > 65536 {
                return Err("invalid PiPL vendor or length".into());
            }
            let payload = bytes(data, at + 16, len)?;
            let string = || -> Result<String, String> {
                let (&n, tail) = payload.split_first().ok_or("empty PiPL string")?;
                let s = if usize::from(n) <= tail.len() && tail.get(usize::from(n)..).is_some_and(|padding| padding.iter().all(|b| *b == 0)) {
                    bytes(tail, 0, usize::from(n))?
                } else {
                    let n = payload.iter().position(|b| *b == 0).ok_or("unterminated PiPL identity")?;
                    if !payload.get(n..).is_some_and(|p| p.iter().all(|b| *b == 0)) {
                        return Err("ambiguous PiPL string encoding".into());
                    }
                    bytes(payload, 0, n)?
                };
                if s.len() > 1024 {
                    return Err("PiPL identity string exceeds 1024 bytes".into());
                }
                String::from_utf8(s.to_vec()).map_err(|_| "non-UTF8 PiPL identity".into())
            };
            match &key {
                b"kind" => {
                    let value = read(at + 16)?.to_be_bytes();
                    effect.kind = String::from_utf8_lossy(&value).into_owned();
                }
                b"name" => effect.name = string()?,
                b"catg" => effect.category = string()?,
                b"eMNA" => effect.match_name = string()?,
                b"eVER" => effect.version_raw = Some(read(at + 16)?),
                _ => {}
            }
            at = at.checked_add(16).and_then(|n| n.checked_add(len)).and_then(|n| n.checked_add(3)).map(|n| n & !3).ok_or("PiPL record overflow")?;
        }
        if !effect.kind.is_empty() {
            return Ok(effect);
        }
    }
    Err("unrecognized PiPL header/kind; identity remains unknown".into())
}

pub fn inspect(data: &[u8]) -> Result<Metadata, String> {
    let image = Image::new(data)?;
    let mut out = Metadata { architecture: "AMD64/PE32+".into(), ..Default::default() };
    for (field, directory) in [(&mut out.imports, 1), (&mut out.delay_imports, 13)] {
        match image.imports(directory) {
            Ok(v) => *field = v,
            Err(e) => out.diagnostics.push(e),
        }
    }
    match image.exports() {
        Ok(v) => out.exports = v,
        Err(e) => out.diagnostics.push(e),
    }
    if let Err(e) = image.resources(&mut out) {
        out.diagnostics.push(e);
    }
    Ok(out)
}
