//! Typed native bindings share the editor's registry; binary decoding remains external.
use crate::{Report, Result, map_property};
use aep_reader as ae;
use effectcraft_effects::after_effects::{self, Conversion, ParameterBinding};
use effectcraft_keyframe::{Interp, Value};
use effectcraft_project::{ParamUi, PropGroup, build};
use serde_json::{Value as Json, json};
use std::collections::BTreeSet;

pub(crate) struct Mapped {
    pub groups: Vec<PropGroup>,
    pub normalized_points: Vec<Json>,
    pub resolution: Vec<Json>,
    pub unavailable: Vec<String>,
}

struct Resolved {
    effect_id: &'static str,
    parameters: Vec<(String, ParameterBinding)>,
    source: &'static str,
}

fn resolve(node: &ae::PropertyNode) -> Option<Resolved> {
    let (_, effect_id) = after_effects::identity(&node.match_name)?;
    if let Some(binding) = after_effects::binding(&node.match_name) {
        return Some(Resolved {
            effect_id,
            parameters: binding.parameters.iter().map(|p| (format!("{}-{}", binding.match_name, p.suffix), *p)).collect(),
            source: "explicit-match-name-binding",
        });
    }
    let parameters = node
        .children
        .iter()
        .filter_map(|n| {
            let definition = n.property.as_ref()?.effect_definition.as_ref()?;
            if definition.match_name != n.match_name {
                return None;
            }
            let binding =
                after_effects::definition_binding(&node.match_name, &n.match_name, &definition.name, definition.control_type, &definition.enum_options)?;
            Some((n.match_name.clone(), binding))
        })
        .collect();
    Some(Resolved { effect_id, parameters, source: "experimental-saved-definition-binding" })
}

fn builtin_default(node: &ae::PropertyNode) -> bool {
    node.children.iter().all(|c| match c.match_name.as_str() {
        "ADBE Effect Mask Parade" => c.children.is_empty(),
        "ADBE Effect Mask Opacity" => c.property.as_ref().is_some_and(|p| {
            p.keyframes.is_empty()
                && p.expression.is_none()
                && p.static_value.as_ref().and_then(ae::Value::numbers).is_some_and(|v| matches!(v,[n] if n.value==Some(100.0)))
        }),
        "ADBE Force CPU GPU" => true,
        _ => false,
    })
}

fn parameter(
    source: &ae::Property,
    target: &mut effectcraft_project::Property,
    binding: &ParameterBinding,
    context: &str,
    ticks: u32,
    report: &mut Report,
) -> Result<()> {
    let mut prepared = target.clone();
    let popup = matches!(binding.conversion, Conversion::Popup | Conversion::WarpStyle);
    if popup {
        prepared.value = Value::Scalar(f64::from(target.value.as_enum()) + 1.0);
    }
    map_property(source, &mut prepared, "native-effect-parameter", ticks, context, report)?;
    if popup {
        prepared.value = after_effects::convert(binding, &[prepared.value.as_f64()], &target.value)?;
        for key in &mut prepared.keys {
            key.value = after_effects::convert(binding, &[key.value.as_f64()], &target.value)?;
            key.in_interp = Interp::Hold;
            key.out_interp = Interp::Hold;
        }
    }
    if binding.conversion == Conversion::ColorArgb255 {
        for key in &mut prepared.keys {
            for ease in [&mut key.in_ease, &mut key.out_ease] {
                if ease.len() == 4 {
                    ease.rotate_left(1);
                }
                for e in ease {
                    e.speed /= 255.0;
                }
            }
        }
    }
    let validate = |value: &Value| -> Result<()> {
        match (&target.ui, value) {
            (ParamUi::Slider { min, max, .. }, Value::Scalar(v)) if v < min || v > max => {
                Err("saved value is outside the native implementation's supported range; preserved without clamping".into())
            }
            (ParamUi::Popup { options }, Value::Enum(index)) if usize::try_from(*index).ok().is_none_or(|i| i >= options.len()) => {
                Err("saved popup is outside the native option table".into())
            }
            _ => Ok(()),
        }
    };
    validate(&prepared.value)?;
    for key in &prepared.keys {
        validate(&key.value)?;
    }
    *target = prepared;
    Ok(())
}

fn instance(
    node: &ae::PropertyNode,
    binding: &Resolved,
    ids: &mut build::Ids<'_>,
    ticks: u32,
    index: usize,
    mapped: &mut BTreeSet<usize>,
    report: &mut Report,
) -> Result<(PropGroup, Vec<Json>, Vec<Json>)> {
    if node.children.iter().any(|c| c.match_name == "ADBE Effect Built In Params" && !builtin_default(c)) {
        return Err("non-default effect masks/compositing options are preserved but not applied".into());
    }
    let spec = effectcraft_effects::find(binding.effect_id).ok_or("native implementation is unavailable")?;
    let mut group = effectcraft_effects::instantiate(spec, ids, &node.name, [1.0, 1.0]);
    group.enabled = node.enabled.unwrap_or(true);
    let mut points = Vec::new();
    let mut resolution = Vec::new();
    let mut local = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for p in &spec.params {
        if matches!(p.ui, ParamUi::Point) {
            points.push(json!({"effectIndex":index,"parameter":p.id}));
        }
    }
    for (name, p) in &binding.parameters {
        if !targets.insert(p.target) {
            return Err(format!("ambiguous saved declarations for native parameter {}", p.target));
        }
        let mut matches = node.children.iter().filter(|n| &n.match_name == name);
        let source = matches.next();
        if matches.next().is_some() {
            return Err(format!("duplicate parameter {name}"));
        }
        let target = group.prop_mut(p.target).ok_or_else(|| format!("native parameter missing: {}", p.target))?;
        let Some(source) = source else {
            resolution.push(json!({"matchName":name,"target":p.target,"status":"notPresent","reason":"Native default retained; no source value invented"}));
            continue;
        };
        let property = source.property.as_ref().ok_or_else(|| format!("parameter {name} has no decoded payload"))?;
        if property.static_value.is_none() && property.keyframes.is_empty() {
            return Err(format!("{name} has no decoded value or keys"));
        }
        parameter(property, target, p, &name, ticks, report)?;
        local.insert(source.source.offset);
        resolution.push(json!({"matchName":name,"target":p.target,"status":"mapped","conversion":p.conversion,"units":p.units,"value":target.value,"keyframes":target.keys.len(),"firstKey":target.keys.first(),"lastKey":target.keys.last(),"expressionDisabled":property.expression.is_some(),"sourceTicksPerSecond":property.source_ticks_per_second}));
    }
    if local.is_empty() {
        return Err("no supported saved parameter was mapped".into());
    }
    local.insert(node.source.offset);
    mapped.extend(local);
    Ok((group, points, resolution))
}

pub(crate) fn convert(nodes: &[ae::PropertyNode], ids: &mut build::Ids<'_>, ticks: u32, mapped: &mut BTreeSet<usize>, report: &mut Report) -> Mapped {
    let mut out = Mapped { groups: vec![], normalized_points: vec![], resolution: vec![], unavailable: vec![] };
    for (order, node) in nodes.iter().enumerate().filter(|(_, n)| n.property.is_none()) {
        let Some(binding) = resolve(node) else {
            out.unavailable.push(node.match_name.clone());
            out.resolution
                .push(json!({"matchName":node.match_name,"sourceOrder":order,"status":"unsupported","reason":"No verified native parameter binding"}));
            continue;
        };
        let count = (report.properties, report.keyframes);
        match instance(node, &binding, ids, ticks, out.groups.len(), mapped, report) {
            Ok((group, points, parameters)) => {
                let unbound: Vec<_> = node
                    .children
                    .iter()
                    .filter(|n| n.property.is_some() && !mapped.contains(&n.source.offset) && !n.match_name.ends_with("-0000"))
                    .map(|n| n.match_name.as_str())
                    .collect();
                let expressions = parameters.iter().any(|p| p.get("expressionDisabled") == Some(&Json::Bool(true)));
                out.resolution.push(json!({"matchName":node.match_name,"effectId":binding.effect_id,"sourceOrder":order,"effectIndex":out.groups.len(),"status":"mapped","bindingSource":binding.source,"partial":!unbound.is_empty() || expressions || binding.source!="explicit-match-name-binding","unboundParameters":unbound,"renderAvailable":true,"rendered":false,"visualEquivalenceVerified":false,"parameters":parameters}));
                out.normalized_points.extend(points);
                out.groups.push(group);
            }
            Err(error) => {
                report.properties = count.0;
                report.keyframes = count.1;
                // Keep the prior AEP Gaussian definition-derived fallback available.
                if node.match_name == "ADBE Gaussian Blur 2" {
                    match crate::effects::convert(&ae::Project::default(), std::slice::from_ref(node), ids, ticks, "preset", mapped, report) {
                        Ok(groups) if !groups.is_empty() => {
                            out.resolution.push(json!({"matchName":node.match_name,"effectId":"ec.blur.gaussian","sourceOrder":order,"effectIndex":out.groups.len(),"status":"mapped","bindingSource":"legacy-definition-derived-gaussian","partial":true,"renderAvailable":true,"rendered":false,"visualEquivalenceVerified":false}));
                            out.groups.extend(groups);
                            continue;
                        }
                        _ => {}
                    }
                }
                report.warn(&node.match_name, format!("Native effect not applied: {error}; all saved data preserved"));
                out.unavailable.push(node.match_name.clone());
                out.resolution.push(json!({"matchName":node.match_name,"sourceOrder":order,"status":"unsupported","reason":error}));
            }
        }
    }
    out
}
