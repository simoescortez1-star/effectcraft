//! Edition-owned extensions, compiled only in the separate reader process.
use crate::{Diagnostic, Error, Limits, PropertyNode, error::Budget, rifx::Chunk};
use std::collections::VecDeque;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PresetBinding {
    pub path: Vec<String>,
    pub path_offset: usize,
    pub settings_offset: usize,
}

/// Saved target paths form a queue before the sentinel; settings follow in the same order.
/// A descriptor clock belongs to its own property, not every property in the container.
pub fn effectcraft_preset(bytes: &[u8], fallback_ticks: u32) -> Result<(Vec<PropertyNode>, Vec<Diagnostic>, u32, bool, Vec<PresetBinding>), Error> {
    if bytes.get(..4) != Some(b"RIFX") || bytes.get(8..12) != Some(b"FaFX") || fallback_ticks == 0 {
        return Err(Error::InvalidFormat { offset: 0, message: "expected RIFX/FaFX preset and nonzero clock" });
    }
    let limits = Limits::default();
    let mut budget = Budget::new(&limits);
    Budget::check("input bytes", bytes.len(), limits.max_input_bytes)?;
    let (root, consumed) = crate::rifx::chunk(bytes, 0, bytes.len(), 0, &mut budget)?;
    let mut saved_clock = None;
    for chunk in root.walk().filter(|c| c.id == *b"beso" && c.u32(0) == Some(1) && c.u32(4) == Some(1)) {
        if let Some(rate) = chunk.u32(12).filter(|rate| *rate != 0) {
            if rate > 1_000_000_000 || saved_clock.is_some_and(|previous| previous != rate) {
                return Err(Error::InvalidFormat { offset: chunk.offset, message: "invalid or conflicting container preset timebases" });
            }
            saved_clock = Some(rate);
        }
    }
    let descriptor_clock = root.walk().filter(|c| matches!(&c.id, b"tdb4" | b"Tdb4")).find_map(|c| c.u32(12).filter(|r| *r > 0 && *r <= 1_000_000_000));
    let ticks = saved_clock.or(descriptor_clock).unwrap_or(fallback_ticks);
    let mut result = Vec::new();
    let mut diagnostics = Vec::new();
    let mut bindings = Vec::new();
    if consumed < bytes.len() {
        diagnostics.push(Diagnostic {
            offset: consumed,
            code: "preset-trailing-data".into(),
            message: format!("{} trailing bytes are preserved in the original source, without assigning semantics", bytes.len() - consumed),
        });
    }
    for block in root.find_all_lists(b"besc") {
        let mut paths = VecDeque::new();
        for child in &block.children {
            if child.form == Some(*b"tdsp") {
                let path: Vec<_> = child.find_all(b"tdmn").filter(|n| n.text() != "ADBE End of path sentinel").collect();
                if !path.is_empty() {
                    paths.push_back((child.offset, path));
                }
            } else if matches!(child.form.as_ref(), Some(b"sspc" | b"tdbs" | b"otst" | b"om-s" | b"GCst")) {
                let (offset, path) =
                    paths.pop_front().ok_or(Error::InvalidFormat { offset: child.offset, message: "preset settings have no queued target path" })?;
                let name = path.last().copied().ok_or(Error::Allocation)?;
                let named = Chunk { id: name.id, form: None, offset: name.offset, data: name.data, children: Vec::new(), opaque: false };
                let (mut settings, _) = crate::rifx::chunk(bytes, child.offset, bytes.len(), 0, &mut budget)?;
                if settings.form == Some(*b"sspc") {
                    if let Some(index) = settings.children.iter().position(|c| c.form == Some(*b"tdbs")) {
                        settings = settings.children.remove(index);
                    }
                }
                let pair = Chunk { id: *b"LIST", form: Some(*b"tdgp"), offset: block.offset, data: &[], children: vec![named, settings], opaque: false };
                crate::decode::nodes(&pair, &mut result, ticks, &mut budget, &mut diagnostics)?;
                bindings.push(PresetBinding { path: path.iter().map(|c| c.text().into_owned()).collect(), path_offset: offset, settings_offset: child.offset });
            }
        }
        for (offset, _) in paths {
            diagnostics.push(Diagnostic {
                offset,
                code: "preset-unpaired-path".into(),
                message: "Target path has no settings block; original bytes preserved".into(),
            });
        }
    }
    if result.is_empty() {
        return Err(Error::InvalidFormat { offset: 12, message: "no supported preset property settings blocks" });
    }
    fn indices(nodes: &mut [PropertyNode], depth: usize, limit: usize) -> Result<(), Error> {
        if depth > limit {
            return Err(Error::InvalidFormat { offset: 0, message: "preset property depth exceeds limit" });
        }
        for (index, node) in nodes.iter_mut().enumerate() {
            node.index = index;
            node.depth = depth;
            indices(&mut node.children, depth + 1, limit)?;
        }
        Ok(())
    }
    indices(&mut result, 0, limits.max_depth)?;
    Ok((result, diagnostics, ticks, saved_clock.or(descriptor_clock).is_some(), bindings))
}
