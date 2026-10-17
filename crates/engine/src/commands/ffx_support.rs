//! Target-dependent conversions for the bounded, data-only FFX carrier.
use effectcraft_keyframe::Value;
use effectcraft_project::{Layer, Node, ParamUi, PropGroup, Property};

pub(super) fn ensure_transform(layer: &mut Layer, path: &str, next: &mut u64) -> std::result::Result<(), String> {
    if layer.props.prop(path).is_some() {
        return Ok(());
    }
    let leaf = path.strip_prefix("transform/").ok_or("invalid transform path")?;
    let transform = layer.props.sub_mut("transform").ok_or("target has no transform group")?;
    if matches!(leaf, "positionX" | "positionY" | "positionZ") {
        let position = transform.get("position").ok_or("target has no Position")?.clone();
        for (axis, (id, name)) in [("positionX", "X Position"), ("positionY", "Y Position"), ("positionZ", "Z Position")].iter().enumerate() {
            if transform.get(id).is_some() {
                continue;
            }
            let uid = *next;
            *next = next.checked_add(1).ok_or("transform ID overflow")?;
            let mut p = Property::new(uid, id, name, Value::Scalar(position.value.components().get(axis).copied().unwrap_or(0.0)));
            p.three_d_only = axis == 2;
            p.keys = super::link::split_keys(&position.keys, position.spatial, axis);
            transform.children.push(Node::Prop(p));
        }
    } else if matches!(leaf, "skew" | "skewAxis") {
        let uid = *next;
        *next = next.checked_add(1).ok_or("transform ID overflow")?;
        let p = Property::new(uid, leaf, if leaf == "skew" { "Skew" } else { "Skew Axis" }, Value::Scalar(0.0)).with_ui(ParamUi::Angle);
        transform.children.push(Node::Prop(p));
    } else {
        return Err(format!("target layer has no {path}"));
    }
    Ok(())
}

pub(super) fn resolve_point(group: &mut PropGroup, parameter: &str, size: [f64; 2]) -> std::result::Result<(), String> {
    if !effectcraft_effects::after_effects::point_binding_allowed(&group.match_id, parameter) {
        return Err("point conversion is not declared by the native binding".into());
    }
    if size.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err("target has no valid effect-space dimensions".into());
    }
    let p = group.prop_mut(parameter).ok_or("missing point conversion parameter")?;
    let scale = |v: &mut Value| -> std::result::Result<(), String> {
        let Value::Vec2(v) = v else { return Err("normalized point must have two components".into()) };
        for (value, scale) in v.iter_mut().zip(size) {
            *value *= scale;
            if !value.is_finite() {
                return Err("point conversion overflow".into());
            }
        }
        Ok(())
    };
    scale(&mut p.value)?;
    let old = p.keys.clone();
    for k in &mut p.keys {
        scale(&mut k.value)?;
        for tangent in [&mut k.spatial_in, &mut k.spatial_out] {
            for (v, s) in tangent.iter_mut().zip(size) {
                *v *= s;
            }
        }
        if !p.spatial {
            for ease in [&mut k.in_ease, &mut k.out_ease] {
                for (e, s) in ease.iter_mut().zip(size) {
                    e.speed *= s;
                }
            }
        }
    }
    if p.spatial {
        let ratio = |segment| {
            let before = effectcraft_keyframe::spatial_segment_length(&old, segment);
            if before > 1e-15 { effectcraft_keyframe::spatial_segment_length(&p.keys, segment) / before } else { 1.0 }
        };
        let ratios: Vec<_> = (0..p.keys.len().saturating_sub(1)).map(ratio).collect();
        for (i, k) in p.keys.iter_mut().enumerate() {
            let incoming = i.checked_sub(1).and_then(|j| ratios.get(j)).copied().unwrap_or(1.0);
            let outgoing = ratios.get(i).copied().unwrap_or(1.0);
            for e in &mut k.in_ease {
                e.speed *= incoming;
            }
            for e in &mut k.out_ease {
                e.speed *= outgoing;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_centers_resolve_against_each_actual_layer_not_the_carrier() {
        let spec = effectcraft_effects::find("ec.stylize.motiontile").unwrap();
        let mut next = 1;
        let mut g = effectcraft_effects::instantiate(spec, &mut effectcraft_project::build::Ids(&mut next), "Tile", [256.0, 256.0]);
        g.prop_mut("tileCenter").unwrap().value = Value::Vec2([0.25, 0.75]);
        resolve_point(&mut g, "tileCenter", [80.0, 40.0]).unwrap();
        assert_eq!(g.prop("tileCenter").unwrap().value, Value::Vec2([20.0, 30.0]));
        assert!(resolve_point(&mut g, "outputWidth", [80.0, 40.0]).is_err());
    }
}
