//! Edition-owned data-only preset adapter. This file is compiled outside the editor.
use crate::*;
use effectcraft_project::Node;
use std::fmt::Write;

pub(crate) fn preset(bytes: &[u8]) -> Result<Import> {
    if bytes.len() > 1024 * 1024 {
        return Err("FFX exceeds the experimental 1 MiB limit".into());
    }
    // The source does not expose a verified rate field. Retain raw ticks and bytes
    // so this explicitly reported assumption can be corrected without data loss.
    const CLOCK: u32 = 8000;
    let (nodes, diagnostics) = ae::effectcraft_preset(bytes, CLOCK).map_err(|e| e.to_string())?;
    let mut project = Project::default();
    let mut report = Report::default();
    report.warn("preset", "Experimental FFX clock: 8000 ticks/second assumed, not verified against AE. Raw ticks, parameters and source bytes are preserved. Expressions remain disabled. Unsupported effects are preserved as data, never approximated.");
    for diagnostic in diagnostics {
        report.warn(&format!("byte {}", diagnostic.offset), format!("{}: {}", diagnostic.code, diagnostic.message));
    }
    let mut comp = Comp::new(256, 256, FrameRate::new(30, 1), seconds(3600.0)?);
    let mut layer = build::layer(&mut project, &comp, "FFX carrier", LayerSource::Null, (256, 256), None);
    let mut paths = Vec::new();
    let mut mapped = BTreeSet::new();
    fn visit<'a>(nodes: &'a [ae::PropertyNode], all: &mut Vec<&'a ae::PropertyNode>) {
        for node in nodes {
            all.push(node);
            visit(&node.children, all);
        }
    }
    let mut all = Vec::new();
    visit(&nodes, &mut all);
    for (name, path) in TRANSFORMS {
        let matches: Vec<_> = all.iter().filter(|n| n.match_name == *name && n.property.is_some()).collect();
        if matches.len() > 1 {
            return Err(format!("ambiguous preset property {name}"));
        }
        if let Some(node) = matches.first() {
            let property = node.property.as_ref().ok_or("missing preset property")?;
            let target = layer.props.prop_mut(path).ok_or("missing carrier transform")?;
            map_property(property, target, name, CLOCK, "preset", &mut report)?;
            mapped.insert(node.source.offset);
            paths.push(*path);
        }
    }
    // Existing Gaussian mapper is shared with AEP conversion. All other effect
    // identities remain unavailable, together with their complete saved tree.
    let effects = effects::convert(&ae::Project::default(), &nodes, &mut build::Ids(&mut project.next_id), CLOCK, "preset", &mut mapped, &mut report)?;
    let available = effects.iter().map(|g| g.match_id.clone()).collect::<Vec<_>>();
    if let Some(group) = layer.props.sub_mut("effects") {
        group.children.extend(effects.into_iter().map(Node::Group));
    }
    let unavailable: Vec<_> = nodes.iter().filter(|n| !mapped.contains(&n.source.offset) && n.property.is_none()).map(|n| n.match_name.clone()).collect();
    for name in &unavailable {
        report.warn(name, "Required effect/runtime unavailable; saved parameters and animation preserved without changing rendered pixels.");
    }
    report_unmapped(&nodes, "preset", &mapped, &mut report);
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").map_err(|e| e.to_string())?;
    }
    layer.comment = serde_json::to_string(&serde_json::json!({"effectcraftPreset":1,"assumedTicksPerSecond":CLOCK,"timingVerified":false,"sourceHex":hex,"nodes":nodes,"mappedPaths":paths,"availableEffects":available,"unavailableEffects":unavailable})).map_err(|e|e.to_string())?;
    if layer.comment.len() > 6 * 1024 * 1024 {
        return Err("decoded preset metadata exceeds 6 MiB".into());
    }
    comp.layers.push(layer);
    project.add_item("FFX carrier", Label::None, None, ItemKind::Comp(Arc::new(comp)));
    report.compositions = 1;
    report.layers = 1;
    Ok(Import { project, report })
}
