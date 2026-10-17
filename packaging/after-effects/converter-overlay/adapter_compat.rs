//! Edition-owned data-only preset adapter. This file is compiled outside the editor.
use crate::*;
use effectcraft_project::Node;
use std::fmt::Write;

pub(crate) fn preset(bytes: &[u8]) -> Result<Import> {
    if bytes.len() > 1024 * 1024 {
        return Err("FFX exceeds the experimental 1 MiB limit".into());
    }
    let (nodes, diagnostics, clock, saved_clock, bindings) = ae::effectcraft_preset(bytes, 8000).map_err(|e| e.to_string())?;
    let mut project = Project::default();
    let mut report = Report::default();
    if saved_clock {
        report.warn("preset", format!("FFX container/fallback timebase: {clock} ticks/second. Each property's nonzero saved descriptor rate controls its own keys. Saved key offsets are retained relative to the playhead. AE reference timing and visual equivalence remain unverified. Expressions remain disabled."));
    } else {
        report.warn("preset", "Experimental FFX clock: 8000 ticks/second assumed because no saved timebase was found. Raw ticks, parameters and source bytes are preserved; AE timing is unverified. Expressions remain disabled.");
    }
    for diagnostic in diagnostics {
        report.warn(&format!("byte {}", diagnostic.offset), format!("{}: {}", diagnostic.code, diagnostic.message));
    }
    let mut comp = Comp::new(256, 256, FrameRate::new(30, 1), seconds(3600.0)?);
    let mut layer = build::layer(&mut project, &comp, "FFX carrier", LayerSource::Null, (256, 256), None);
    let mut paths = Vec::new();
    let mut transform_dimensions = serde_json::Map::new();
    let mut mapped = BTreeSet::new();
    fn visit<'a>(nodes: &'a [ae::PropertyNode], all: &mut Vec<&'a ae::PropertyNode>) {
        for node in nodes {
            if node.property.is_some() {
                all.push(node);
            }
            if node.match_name == "ADBE Transform Group" {
                visit(&node.children, all);
            }
        }
    }
    let mut all = Vec::new();
    visit(&nodes, &mut all);
    for (name, path) in effectcraft_effects::after_effects::TRANSFORMS {
        let matches: Vec<_> = all.iter().filter(|n| n.match_name == *name && n.property.is_some()).collect();
        if matches.len() > 1 {
            report.warn(name, "Ambiguous duplicate transform; this property remains preserved while other mappings continue");
            continue;
        }
        if let Some(node) = matches.first() {
            let property = node.property.as_ref().ok_or("missing preset property")?;
            if matches!(*name, "ADBE Rotate X" | "ADBE Rotate Y" | "ADBE Orientation" | "ADBE Position_2") {
                report.warn(
                    name,
                    "This channel renders only on a 3D target layer. The preset preserves the layer's 3D switch; enable 3D explicitly when needed.",
                );
            }
            if property.static_value.is_none() && property.keyframes.is_empty() {
                report.warn(name, "Transform has no decoded value or keys; no default value is applied");
                continue;
            }
            if *name == "ADBE Skew"
                && property
                    .static_value
                    .iter()
                    .chain(property.keyframes.iter().filter_map(|k| k.value.as_ref()))
                    .filter_map(ae::Value::numbers)
                    .flatten()
                    .any(|n| n.value.is_none_or(|v| v.abs() > 85.0))
            {
                report.warn(name, "Skew exceeds the native supported range -85..85 degrees; preserved without clamping");
                continue;
            }
            if layer.props.prop(path).is_none()
                && matches!(*path, "transform/skew" | "transform/skewAxis" | "transform/positionX" | "transform/positionY" | "transform/positionZ")
            {
                let leaf = path.strip_prefix("transform/").ok_or("invalid additional transform path")?;
                let mut p = build::Ids(&mut project.next_id).prop(leaf, name, effectcraft_keyframe::Value::Scalar(0.0));
                if leaf.starts_with("skew") {
                    p.ui = effectcraft_project::ParamUi::Angle;
                }
                layer.props.sub_mut("transform").ok_or("missing transform group")?.children.push(Node::Prop(p));
            }
            let target = layer.props.prop_mut(path).ok_or("missing carrier transform")?;
            let mut prepared = target.clone();
            if let Err(error) = map_property(property, &mut prepared, name, clock, "preset", &mut report) {
                report.warn(name, format!("Transform not applied: {error}; original data preserved"));
                continue;
            }
            *target = prepared;
            mapped.insert(node.source.offset);
            paths.push(*path);
            transform_dimensions.insert((*path).to_string(), serde_json::json!(property.storage_dimensions));
        }
    }
    let native = effectcraft_native_effects::convert(&nodes, &mut build::Ids(&mut project.next_id), clock, &mut mapped, &mut report);
    let available = native.groups.iter().map(|g| g.match_id.clone()).collect::<Vec<_>>();
    if let Some(group) = layer.props.sub_mut("effects") {
        group.children.extend(native.groups.into_iter().map(Node::Group));
    }
    let unavailable = native.unavailable;
    for name in &unavailable {
        report.warn(name, "Saved effect not applied: native implementation or a supported parameter mapping is unavailable. Inspect the exact resolver reason. Original values, animation and bytes are preserved.");
    }
    report_unmapped(&nodes, "preset", &mapped, &mut report);
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").map_err(|e| e.to_string())?;
    }
    layer.comment = serde_json::to_string(&serde_json::json!({"effectcraftPreset":1,"sourceTicksPerSecond":clock,"assumedTicksPerSecond":(!saved_clock).then_some(clock),"timingSource":if saved_clock {"saved-container/property-metadata"} else {"legacy-assumption"},"timingVerified":false,"sourceHex":hex,"nodes":nodes,"bindings":bindings,"mappedPaths":paths,"transformDimensions":transform_dimensions,"availableEffects":available,"unavailableEffects":unavailable,"effectResolution":native.resolution,"normalizedPointParameters":native.normalized_points})).map_err(|e|e.to_string())?;
    if layer.comment.len() > 6 * 1024 * 1024 {
        return Err("decoded preset metadata exceeds 6 MiB".into());
    }
    comp.layers.push(layer);
    project.add_item("FFX carrier", Label::None, None, ItemKind::Comp(Arc::new(comp)));
    report.compositions = 1;
    report.layers = 1;
    Ok(Import { project, report })
}
