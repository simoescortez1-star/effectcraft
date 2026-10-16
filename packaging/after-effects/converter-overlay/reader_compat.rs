//! Edition-owned extensions, compiled only in the separate reader process.
use crate::{Diagnostic, Error, Limits, PropertyNode, error::Budget, rifx::Chunk};

/// Decode the saved property tree of a FaFX preset without evaluating expressions.
/// Timing defaults to the historical 8000-tick preset clock; callers retain that assumption.
pub fn effectcraft_preset(bytes: &[u8], ticks: u32) -> Result<(Vec<PropertyNode>, Vec<Diagnostic>), Error> {
    if bytes.get(..4) != Some(b"RIFX") || bytes.get(8..12) != Some(b"FaFX") || ticks == 0 {
        return Err(Error::InvalidFormat { offset: 0, message: "expected RIFX/FaFX preset and nonzero clock" });
    }
    let limits = Limits::default();
    let mut budget = Budget::new(&limits);
    Budget::check("input bytes", bytes.len(), limits.max_input_bytes)?;
    let (root, _) = crate::rifx::chunk(bytes, 0, bytes.len(), 0, &mut budget)?;
    let mut result = Vec::new();
    let mut diagnostics = Vec::new();
    for block in root.find_all_lists(b"besc") {
        let mut path = Vec::new();
        for child in &block.children {
            if child.form == Some(*b"tdsp") {
                let next: Vec<_> = child.find_all(b"tdmn").filter(|n| n.text() != "ADBE End of path sentinel").collect();
                if !next.is_empty() {
                    path = next;
                }
            } else if child.form == Some(*b"sspc") {
                // Some files save an end-of-path marker between path and settings.
                let name = path.last().copied().ok_or(Error::InvalidFormat { offset: child.offset, message: "preset settings have no named target" })?;
                let named = Chunk { id: name.id, form: None, offset: name.offset, data: name.data, children: Vec::new(), opaque: false };
                // Borrow the settings chunk by moving a separately parsed copy of this one subtree.
                let (mut settings, _) = crate::rifx::chunk(bytes, child.offset, bytes.len(), 0, &mut budget)?;
                if let Some(index) = settings.children.iter().position(|c| c.form == Some(*b"tdbs")) {
                    settings = settings.children.remove(index);
                }
                let pair = Chunk { id: *b"LIST", form: Some(*b"tdgp"), offset: block.offset, data: &[], children: vec![named, settings], opaque: false };
                crate::decode::nodes(&pair, &mut result, ticks, &mut budget, &mut diagnostics)?;
            }
        }
    }
    if result.is_empty() {
        return Err(Error::InvalidFormat { offset: 12, message: "no supported preset property settings blocks" });
    }
    Ok((result, diagnostics))
}
