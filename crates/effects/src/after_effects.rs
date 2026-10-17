//! Explicit AE match-name bindings. Availability, import mapping and visual equivalence differ.
//! Parameter declarations in a preset are retained separately; display labels never select a binding.
use effectcraft_keyframe::Value;
use serde::Serialize;
use serde_json::{Value as Json, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Conversion {
    Number,
    NormalizedPoint,
    Boolean,
    ColorArgb255,
    Popup,
    WarpStyle,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ParameterBinding {
    pub suffix: &'static str,
    pub target: &'static str,
    pub conversion: Conversion,
    pub units: &'static str,
}

#[derive(Debug, Serialize)]
pub struct EffectBinding {
    pub match_name: &'static str,
    pub effect_id: &'static str,
    pub parameters: &'static [ParameterBinding],
}

const fn p(suffix: &'static str, target: &'static str, conversion: Conversion, units: &'static str) -> ParameterBinding {
    ParameterBinding { suffix, target, conversion, units }
}

pub const BINDINGS: &[EffectBinding] = &[
    EffectBinding {
        match_name: "ADBE WRPMESH",
        effect_id: "ec.distort.warp",
        parameters: &[
            p("0001", "warpStyle", Conversion::WarpStyle, "one-based popup"),
            p("0002", "warpAxis", Conversion::Popup, "one-based popup"),
            p("0003", "bend", Conversion::Number, "percent"),
            p("0004", "horizontalDistortion", Conversion::Number, "percent"),
            p("0005", "verticalDistortion", Conversion::Number, "percent"),
        ],
    },
    EffectBinding {
        match_name: "ADBE Tile",
        effect_id: "ec.stylize.motiontile",
        parameters: &[
            p("0001", "tileCenter", Conversion::NormalizedPoint, "normalized layer coordinates; resolved on target"),
            p("0002", "tileWidth", Conversion::Number, "percent"),
            p("0003", "tileHeight", Conversion::Number, "percent"),
            p("0004", "outputWidth", Conversion::Number, "percent"),
            p("0005", "outputHeight", Conversion::Number, "percent"),
            p("0006", "mirrorEdges", Conversion::Boolean, "boolean"),
            p("0007", "phase", Conversion::Number, "degrees"),
            p("0008", "horizontalPhaseShift", Conversion::Boolean, "boolean"),
        ],
    },
    EffectBinding {
        match_name: "ADBE Tint",
        effect_id: "ec.color.tint",
        parameters: &[
            p("0001", "black", Conversion::ColorArgb255, "stored ARGB / 255"),
            p("0002", "white", Conversion::ColorArgb255, "stored ARGB / 255"),
            p("0003", "amount", Conversion::Number, "percent"),
        ],
    },
    EffectBinding {
        match_name: "ADBE Gaussian Blur 2",
        effect_id: "ec.blur.gaussian",
        parameters: &[
            p("0001", "blurriness", Conversion::Number, "pixels"),
            p("0002", "dimensions", Conversion::Popup, "one-based popup"),
            p("0003", "repeatEdge", Conversion::Boolean, "boolean"),
        ],
    },
];

pub const TRANSFORMS: &[(&str, &str)] = &[
    ("ADBE Anchor Point", "transform/anchor"),
    ("ADBE Position", "transform/position"),
    ("ADBE Scale", "transform/scale"),
    ("ADBE Rotate Z", "transform/rotation"),
    ("ADBE Rotate X", "transform/rotationX"),
    ("ADBE Rotate Y", "transform/rotationY"),
    ("ADBE Orientation", "transform/orientation"),
    ("ADBE Opacity", "transform/opacity"),
    ("ADBE Skew", "transform/skew"),
    ("ADBE Skew Axis", "transform/skewAxis"),
    ("ADBE Position_0", "transform/positionX"),
    ("ADBE Position_1", "transform/positionY"),
    ("ADBE Position_2", "transform/positionZ"),
];

pub fn transform_registry() -> Json {
    json!(TRANSFORMS.iter().map(|(name,path)| {
        let percent=matches!(*name,"ADBE Scale"|"ADBE Opacity");
        let vector=matches!(*name,"ADBE Anchor Point"|"ADBE Position"|"ADBE Scale"|"ADBE Orientation");
        json!({"matchName":name,"targetPath":path,"valueType":if vector{"vector2-or-3"}else{"scalar"},"conversionFactor":if percent{100.0}else{1.0},"keyframes":true,
            "requiresThreeD":matches!(*name,"ADBE Rotate X"|"ADBE Rotate Y"|"ADBE Orientation"|"ADBE Position_2"),"visualEquivalenceVerified":false})
    }).collect::<Vec<_>>())
}

pub fn binding(match_name: &str) -> Option<&'static EffectBinding> {
    BINDINGS.iter().find(|b| b.match_name == match_name)
}

pub fn identity(match_name: &str) -> Option<(&'static str, &'static str)> {
    PRIORITIES.iter().find(|(name, _)| *name == match_name).copied()
}

fn canonical(label: &str) -> String {
    label.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

/// Optional, conservative discovery from the effect's saved parameter declaration. Requires
/// an allowlisted effect ID, its own stable parameter ID, a unique canonical label and a
/// compatible declared control type. Localized/ambiguous/unknown controls remain unmapped.
pub fn definition_binding(effect: &str, parameter: &str, label: &str, control: u8, options: &[String]) -> Option<ParameterBinding> {
    let (_, id) = identity(effect)?;
    let suffix = parameter.strip_prefix(effect)?.strip_prefix('-')?;
    if suffix.len() != 4 || !suffix.bytes().all(|c| c.is_ascii_digit()) || !label.is_ascii() || label.len() > 128 {
        return None;
    }
    let spec = crate::find(id)?;
    let label = canonical(label);
    if label.is_empty() {
        return None;
    }
    let mut matches = spec.params.iter().filter(|p| canonical(p.name) == label);
    let p = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let (conversion, units) = match (&p.default, &p.ui, control) {
        (Value::Scalar(_), crate::ParamUi::Angle, 1 | 2 | 3 | 10) => (Conversion::Number, "degrees"),
        (Value::Scalar(_), _, 1 | 2 | 3 | 10) => (Conversion::Number, "native numeric units; definition-assisted"),
        (Value::Bool(_), crate::ParamUi::Checkbox, 4) => (Conversion::Boolean, "boolean"),
        (Value::Color(_), crate::ParamUi::Color, 5) => (Conversion::ColorArgb255, "stored ARGB / 255"),
        (Value::Vec2(_), crate::ParamUi::Point, 6) => (Conversion::NormalizedPoint, "normalized layer coordinates; definition-assisted"),
        (Value::Enum(_), crate::ParamUi::Popup { options: native }, 7)
            if native.len() == options.len() && native.iter().zip(options).all(|(a, b)| canonical(a) == canonical(b)) =>
        {
            (Conversion::Popup, "one-based popup; verified option order")
        }
        _ => return None,
    };
    Some(ParameterBinding { suffix: "", target: p.id, conversion, units })
}

pub fn point_binding_allowed(effect_id: &str, parameter: &str) -> bool {
    PRIORITIES.iter().any(|(_, id)| *id == effect_id)
        && crate::find(effect_id).is_some_and(|spec| spec.params.iter().any(|p| p.id == parameter && matches!(p.ui, crate::ParamUi::Point)))
}

/// No coercion from scalar to vector, unknown enum, invalid boolean or non-finite number.
pub fn convert(parameter: &ParameterBinding, source: &[f64], target: &Value) -> Result<Value, String> {
    if source.iter().any(|v| !v.is_finite()) {
        return Err("non-finite saved parameter".into());
    }
    match (parameter.conversion, source, target) {
        (Conversion::Number, [x], Value::Scalar(_)) => Ok(Value::Scalar(*x)),
        (Conversion::Number | Conversion::NormalizedPoint, [x, y], Value::Vec2(_)) => Ok(Value::Vec2([*x, *y])),
        (Conversion::Number, [x, y, z], Value::Vec3(_)) => Ok(Value::Vec3([*x, *y, *z])),
        (Conversion::Boolean, [x], Value::Bool(_)) if matches!(*x, 0.0 | 1.0) => Ok(Value::Bool(*x == 1.0)),
        (Conversion::ColorArgb255, [a, r, g, b], Value::Color(_)) => Ok(Value::Color([r / 255.0, g / 255.0, b / 255.0, a / 255.0])),
        (Conversion::Popup, [x], Value::Enum(_)) if *x >= 1.0 && x.fract() == 0.0 && *x < 256.0 => Ok(Value::Enum((*x as u32) - 1)),
        (Conversion::WarpStyle, [x], Value::Enum(_)) if *x >= 1.0 && x.fract() == 0.0 && *x <= 15.0 => {
            let index = (*x as u32) - 1;
            // Saved Warp table: Twist, Squeeze. Existing engine: Squeeze, Twist.
            Ok(Value::Enum(match index {
                13 => 14,
                14 => 13,
                _ => index,
            }))
        }
        _ => Err("saved parameter type/dimensions do not match the native binding".into()),
    }
}

/// Public behavior-only identities. Definition-assisted mappings are experimental, and never
/// establish support for arbitrary/custom controls or visual equivalence.
pub const PRIORITIES: &[(&str, &str)] = &[
    ("ADBE WRPMESH", "ec.distort.warp"),
    ("ADBE Tile", "ec.stylize.motiontile"),
    ("ADBE Geometry2", "ec.distort.transform"),
    ("ADBE Corner Pin", "ec.distort.cornerpin"),
    ("ADBE Offset", "ec.distort.offset"),
    ("ADBE Mirror", "ec.distort.mirror"),
    ("ADBE Polar Coordinates", "ec.distort.polar"),
    ("ADBE Turbulent Displace", "ec.distort.turbulentdisplace"),
    ("ADBE Gaussian Blur 2", "ec.blur.gaussian"),
    ("ADBE Motion Blur", "ec.blur.directional"),
    ("ADBE Radial Blur", "ec.blur.radial"),
    ("ADBE Box Blur2", "ec.blur.fastbox"),
    ("ADBE Sharpen", "ec.blur.sharpen"),
    ("ADBE Unsharp Mask2", "ec.blur.unsharp"),
    ("ADBE CurvesCustom", "ec.color.curves"),
    ("ADBE Easy Levels2", "ec.color.levels"),
    ("ADBE Exposure2", "ec.color.exposure"),
    ("ADBE HUE SATURATION", "ec.color.huesaturation"),
    ("ADBE Tint", "ec.color.tint"),
    ("ADBE Fill", "ec.generate.fill"),
    ("ADBE Brightness & Contrast 2", "ec.color.brightnesscontrast"),
    ("ADBE Glo2", "ec.stylize.glow"),
    ("ADBE Drop Shadow", "ec.perspective.dropshadow"),
    ("ADBE Venetian Blinds", "ec.transition.venetian"),
    ("CC RepeTile", "ec.stylize.ccrepetile"),
    ("ADBE Ramp", "ec.generate.gradientramp"),
    ("ADBE Fractal Noise", "ec.noise.fractal"),
    ("ADBE Posterize Time", "ec.time.posterizetime"),
];

pub fn registry() -> Json {
    Json::Array(PRIORITIES.iter().map(|(original,id)| {
        let spec=crate::lookup(id);let mapped=binding(original);
        json!({"matchName":original,"effectId":id,"renderAvailable":spec.is_some(),"mappingStatus":if mapped.is_some(){"partial-explicit-bindings"}else{"experimental-saved-definition-bindings"},"visualEquivalenceVerified":false,
            "nativeParameters":spec.map(|s|s.params.iter().map(|p|json!({"id":p.id,"name":p.name,"default":p.default,"ui":p.ui})).collect::<Vec<_>>()),
            "parameters":mapped.map(|m|m.parameters.iter().map(|b| {
                let target=spec.and_then(|s|s.params.iter().find(|p|p.id==b.target));
                json!({"binding":b,"default":target.map(|p|&p.default),"ui":target.map(|p|&p.ui),"keyframes":target.is_some()})
            }).collect::<Vec<_>>()).unwrap_or_default()})
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_reference_real_native_parameters_and_use_exact_match_names() {
        for (_, id) in PRIORITIES {
            assert!(crate::find(id).is_some(), "missing native implementation: {id}");
        }
        for b in BINDINGS {
            let spec = crate::lookup(b.effect_id).unwrap();
            for p in b.parameters {
                assert!(spec.params.iter().any(|s| s.id == p.target), "{} / {}", b.effect_id, p.target);
            }
        }
        assert!(binding("Warp").is_none());
        assert!(binding("ADBE Tile").is_some());
    }
    #[test]
    fn saved_declarations_require_identity_type_and_unambiguous_labels() {
        let p = definition_binding("ADBE Glo2", "ADBE Glo2-0203", "Glow Intensity", 10, &[]).unwrap();
        assert_eq!(p.target, "intensity");
        assert!(definition_binding("S_Shake", "S_Shake-0203", "Glow Intensity", 10, &[]).is_none());
        assert!(definition_binding("ADBE Glo2", "ADBE Glo2-0203", "Glow Intensity", 15, &[]).is_none());
        assert!(definition_binding("ADBE Glo2", "foreign-0203", "Glow Intensity", 10, &[]).is_none());
    }
    #[test]
    fn conversions_keep_value_types_and_saved_popup_order() {
        let warp = &BINDINGS[0].parameters[0];
        assert_eq!(convert(warp, &[14.0], &Value::Enum(0)).unwrap(), Value::Enum(14));
        assert!(convert(warp, &[16.0], &Value::Enum(0)).is_err());
        let boolean = &BINDINGS[1].parameters[5];
        assert_eq!(convert(boolean, &[1.0], &Value::Bool(false)).unwrap(), Value::Bool(true));
        assert!(convert(boolean, &[2.0], &Value::Bool(false)).is_err());
        assert!(convert(boolean, &[f64::NAN], &Value::Bool(false)).is_err());
    }
}
