//! Real user fixtures remain external. These tests verify processing, not AE visual parity.
use crate::Session;
use effectcraft_keyframe::Value;
use effectcraft_project::{ItemId, Node, Project};
use effectcraft_time::Tick;
use serde_json::json;
use std::sync::Arc;

fn target() -> (Session, u64, ItemId) {
    let executable = std::env::var_os("EFFECTCRAFT_AEP_CONVERTER").expect("explicit converter");
    let mut s = Session {
        ae_converter: Some(Arc::new(effectcraft_ae_bridge::process::ExternalConverter {
            executable: executable.into(),
            timeout: std::time::Duration::from_secs(30),
        })),
        ..Default::default()
    };
    Arc::make_mut(&mut s.project).settings.bit_depth = effectcraft_project::BitDepth::Bpc32;
    s.execute("comp.new", json!({"name":"Native FFX regression","width":64,"height":64,"duration":3,"frameRate":60})).unwrap();
    let layer =
        s.execute("layer.newSolid", json!({"name":"Original generated gradient","width":16,"height":16,"color":"#ffffff"})).unwrap()["layer"].as_u64().unwrap();
    s.execute("effect.apply", json!({"effect":"ec.generate.gradientramp","layers":[layer]})).unwrap();
    for (path, value) in
        [("start", json!([0.0, 0.0])), ("end", json!([16.0, 16.0])), ("startColor", json!([1.0, 0.1, 0.0, 1.0])), ("endColor", json!([0.0, 0.1, 1.0, 1.0]))]
    {
        s.execute("prop.set", json!({"layer":layer,"path":format!("effects/#1/{path}"),"value":value})).unwrap();
    }
    s.execute("time.set", json!({"time":0.5})).unwrap();
    let cid = s.active_comp_id().unwrap();
    (s, layer, cid)
}

fn disable(s: &mut Session, id: &str) {
    let cid = s.active_comp_id().unwrap();
    let l = &mut Arc::make_mut(&mut s.project).comp_mut(cid).unwrap().layers[0];
    for n in &mut l.props.sub_mut("effects").unwrap().children {
        if let Node::Group(g) = n
            && g.match_id == id
        {
            g.enabled = false;
        }
    }
}

fn persistence_export_and_report(s: &mut Session, cid: ItemId, layer: u64, time: f64, case: &str) {
    let frame = s.render(cid, Tick::from_seconds_f64(time), Default::default());
    let saved = s.project.to_file_json().unwrap();
    let reopened = Project::from_json(&saved).unwrap();
    let fresh = effectcraft_render::Renderer::new(&reopened, s.footage.as_ref(), Default::default()).comp_frame(cid, Tick::from_seconds_f64(time));
    assert_eq!(frame.data, fresh.data, "reopened native effect pixels");
    let path = std::env::temp_dir().join(format!("cortez-ffx-{case}-{}.psd", std::process::id()));
    s.execute("comp.saveFrameAsPsd", json!({"path":path,"time":time})).unwrap();
    let exported = effectcraft_psd::Psd::parse(std::fs::read(&path).unwrap()).unwrap().composite().unwrap();
    assert_eq!((exported.width, exported.height), (64, 64));
    for (a, b) in exported.data.iter().zip(&frame.data) {
        // PSD decodes straight RGBA; the renderer returns premultiplied RGBA.
        let premultiplied = [a[0] * a[3], a[1] * a[3], a[2] * a[3], a[3]];
        // The PSD writer is 8 bpc: quantize/clip straight HDR channels, then premultiply.
        let expected_alpha = b[3].clamp(0.0, 1.0);
        let expected = if b[3] > 0.0 {
            [
                (b[0] / b[3]).clamp(0.0, 1.0) * expected_alpha,
                (b[1] / b[3]).clamp(0.0, 1.0) * expected_alpha,
                (b[2] / b[3]).clamp(0.0, 1.0) * expected_alpha,
                expected_alpha,
            ]
        } else {
            [0.0; 4]
        };
        for (a, b) in premultiplied.iter().zip(&expected) {
            assert!((a - b).abs() <= 1.0 / 255.0 + 1e-6, "native preview/export differ");
        }
    }
    std::fs::remove_file(path).unwrap();
    let path = std::env::temp_dir().join(format!("cortez-ffx-{case}-{}.json", std::process::id()));
    s.execute("anim.exportAfterEffectsPresetReport", json!({"path":path,"layers":[layer]})).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(report["kind"], "CortezEffectsFFXReport");
    assert!(report["record"]["preset"]["sourceHex"].as_str().unwrap().len() > 100);
    assert_eq!(report["visualEquivalenceVerified"], false);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn imported_effect_ids_remain_unique_after_reopen_and_the_next_effect() {
    struct Carrier(String);
    impl effectcraft_ae_bridge::ProjectConverter for Carrier {
        fn convert(&self, _: &[u8]) -> std::result::Result<effectcraft_ae_bridge::Conversion, String> {
            Ok(effectcraft_ae_bridge::Conversion {
                project_json: self.0.clone(),
                report_json: json!({"report":{"compositions":1,"layers":1,"warnings":[]}}).to_string(),
            })
        }
    }
    struct Input;
    impl crate::Services for Input {
        fn read_file(&self, _: &str) -> std::io::Result<Vec<u8>> {
            Ok(b"original carrier contract fixture, not FFX".to_vec())
        }
        fn write_file(&self, _: &str, _: &[u8]) -> std::io::Result<()> {
            Err(std::io::ErrorKind::PermissionDenied.into())
        }
    }
    let make = || {
        let mut s = Session::default();
        s.execute("comp.new", json!({"width":32,"height":32})).unwrap();
        s.execute("layer.newSolid", json!({"color":"#ffffff"})).unwrap();
        s
    };
    let mut source = make();
    source.execute("effect.apply", json!({"effect":"ec.distort.warp"})).unwrap();
    let cid = source.active_comp_id().unwrap();
    Arc::make_mut(&mut source.project).comp_mut(cid).unwrap().layers[0].comment =
        json!({"effectcraftPreset":1,"mappedPaths":[],"unavailableEffects":[],"sourceHex":"cafe"}).to_string();
    let mut s = make();
    s.services = Arc::new(Input);
    s.ae_converter = Some(Arc::new(Carrier(source.project.to_file_json().unwrap())));
    s.execute("anim.applyAfterEffectsPreset", json!({"path":"contract.ffx"})).unwrap();
    let layer = &s.active_comp().unwrap().layers[0];
    assert!(s.project.next_id > layer.props.max_uid(), "next free ID must follow every imported property/group ID");
    let saved = s.project.to_file_json().unwrap();
    assert_eq!(Project::from_json(&saved).unwrap().to_file_json().unwrap(), saved, "reopening must not repair ID allocation");
    let imported: Vec<_> = layer.props.sub("effects").unwrap().groups().map(|g| g.uid).collect();
    s.execute("effect.apply", json!({"effect":"ec.color.tint"})).unwrap();
    let last = s.active_comp().unwrap().layers[0].props.sub("effects").unwrap().groups().last().unwrap();
    assert!(!imported.contains(&last.uid));
}

#[test]
#[ignore = "requires the pinned converter and original generated protocol fixtures, not AE visual references"]
fn generated_native_ffx_cases_apply_render_export_persist_and_undo() {
    let root = std::path::PathBuf::from(std::env::var_os("EFFECTCRAFT_TEST_FFX").expect("generated fixture path")).parent().unwrap().to_path_buf();
    let cases: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(root.join("native-cases.json")).unwrap()).unwrap();
    for case in &cases {
        assert_eq!(case["authoredByAfterEffects"], false);
        let name = case["file"].as_str().unwrap();
        let path = root.join(name);
        let bytes = std::fs::read(&path).unwrap();
        let (mut s, layer, cid) = target();
        if case["threeD"] == true {
            s.execute("layer.setSwitch", json!({"layer":layer,"switch":"threeD","value":true})).unwrap();
        }
        let before = s.project.to_json();
        let baseline = s.render(cid, Tick::from_seconds_f64(0.5), Default::default());
        let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":path,"layers":[layer]})).unwrap();
        assert_eq!(result["propertiesApplied"], case["properties"], "{name}: transform count");
        assert_eq!(result["effectsApplied"], case["effects"], "{name}: effect count");
        assert_eq!(result["keyframesApplied"], case["keys"], "{name}: key count");
        assert_eq!(result["applied"], true, "{name}");
        if name == "native-hold.ffx" {
            let p = s.active_comp().unwrap().layers[0].props.prop("transform/opacity").unwrap();
            assert_eq!(p.keys[0].out_interp, effectcraft_keyframe::Interp::Hold);
            assert_eq!(p.value_at(Tick::from_seconds_f64(1.0)), Value::Scalar(30.0));
        }
        if name == "native-bezier.ffx" {
            let p = s.active_comp().unwrap().layers[0].props.prop("transform/scale").unwrap();
            assert!((p.value_at(Tick::from_seconds_f64(0.75)).as_vec3()[0] - 72.5).abs() < 1e-8, "saved ease must not become linear");
            assert_eq!(p.keys[1].time, Tick::from_seconds_f64(1.5));
        }
        assert_eq!(!result["unavailableEffects"].as_array().unwrap().is_empty(), case["partial"] == true, "{name}: partial state");
        let after = s.render(cid, Tick::from_seconds_f64(0.5), Default::default());
        let changed = after.data.iter().zip(&baseline.data).filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| (a - b).abs() > 1e-5)).count();
        assert!(changed > 0, "{name}: recognized data must actually change pixels");
        let saved = s.project.to_file_json().unwrap();
        assert!(s.project.next_id > s.active_comp().unwrap().layers[0].props.max_uid(), "{name}: UID allocation");
        persistence_export_and_report(&mut s, cid, layer, 0.5, name);
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.project.to_json(), before, "{name}: undo");
        s.execute("edit.redo", json!({})).unwrap();
        assert!(s.project.to_file_json().unwrap() == saved, "{name}: redo must restore all imported data");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        println!(
            "{name}: {} properties, {} effects, {} keys, {changed} changed pixels; reopen/export/undo passed",
            result["propertiesApplied"], result["effectsApplied"], result["keyframesApplied"]
        );
    }
    println!("{} original generated FFX cases verified; these are not AE visual reference files", cases.len());
}

#[test]
#[ignore = "requires the private user Warp FFX and pinned external converter"]
fn user_warp_ffx_applies_tint_then_fisheye_keys_and_changes_pixels() {
    let path = std::env::var("EFFECTCRAFT_TEST_USER_WARP").expect("explicit private Warp preset");
    let original = std::fs::read(&path).unwrap();
    let (mut s, layer, cid) = target();
    let before = s.project.to_json();
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":path,"layers":[layer]})).unwrap();
    assert_eq!(result["effectsApplied"], 2);
    assert_eq!(result["effectKeyframesApplied"], 4);
    assert_eq!(result["unavailableEffects"], json!([]));
    let l = &s.active_comp().unwrap().layers[0];
    let groups = l.props.sub("effects").unwrap().groups().collect::<Vec<_>>();
    assert_eq!(groups.iter().map(|g| g.match_id.as_str()).collect::<Vec<_>>(), ["ec.generate.gradientramp", "ec.color.tint", "ec.distort.warp"]);
    assert_eq!(groups[2].prop("warpStyle").unwrap().value, Value::Enum(11));
    let bend = groups[2].prop("bend").unwrap();
    assert_eq!(bend.keys[0].value, Value::Scalar(12.0));
    assert_eq!(bend.keys[1].time, Tick::from_seconds_f64(0.5 + 1.0 / 30.0));
    assert!(!bend.expr.as_ref().unwrap().enabled);
    let changed = s.render(cid, Tick::from_seconds_f64(0.5), Default::default());
    let saved = s.project.to_file_json().unwrap();
    disable(&mut s, "ec.distort.warp");
    let unwarped = s.render(cid, Tick::from_seconds_f64(0.5), Default::default());
    let pixels = changed.data.iter().zip(&unwarped.data).filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| (a - b).abs() > 1e-5)).count();
    assert!(pixels > 8, "Warp must deform the image independently of Tint: {pixels}");
    println!("Warp changed {pixels} pixels independently of Tint; expression remains disabled");
    s.project = Arc::new(Project::from_json(&saved).unwrap());
    persistence_export_and_report(&mut s, cid, layer, 0.5, "warp");
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.project.to_json(), before);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(s.project.to_file_json().unwrap(), saved);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
#[ignore = "requires the private user Scale + Motion Tile FFX and pinned external converter"]
fn user_scale_motion_tile_ffx_preserves_curves_and_renders_expanded_mirrored_pixels() {
    let path = std::env::var("EFFECTCRAFT_TEST_USER_TILE").expect("explicit private combined preset");
    let original = std::fs::read(&path).unwrap();
    let (mut s, layer, cid) = target();
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":path,"layers":[layer]})).unwrap();
    assert_eq!(result["propertiesApplied"], 1);
    assert_eq!(result["keyframesApplied"], 4);
    assert_eq!(result["effectsApplied"], 1);
    let l = &s.active_comp().unwrap().layers[0];
    let scale = l.props.prop("transform/scale").unwrap();
    for (key, tick, value) in [(0, 0, [200.0, 100.0, 0.0]), (1, 5910, [95.0, 100.0, 0.0]), (2, 9671, [105.0, 100.0, 100.0]), (3, 14507, [100.0, 100.0, 100.0])]
    {
        assert_eq!(scale.keys[key].time.0, Tick::from_seconds_f64(0.5).0 + (i64::from(tick) * effectcraft_time::TICKS_PER_SECOND) / 25600);
        for (a, b) in scale.keys[key].value.as_vec3().iter().zip(value) {
            assert!((a - b).abs() < 1e-9);
        }
        assert_eq!(scale.keys[key].out_interp, effectcraft_keyframe::Interp::Bezier);
    }
    assert!((scale.keys[0].out_ease[0].influence - 0.001).abs() < 1e-9);
    assert!((scale.keys[1].in_ease[0].influence - 0.59849286).abs() < 1e-7);
    let tile = l.props.sub("effects").unwrap().groups().last().unwrap();
    assert_eq!(tile.match_id, "ec.stylize.motiontile");
    assert_eq!(tile.prop("tileCenter").unwrap().value, Value::Vec2([8.0, 8.0]));
    assert_eq!(tile.prop("outputWidth").unwrap().value, Value::Scalar(410.0));
    assert_eq!(tile.prop("outputHeight").unwrap().value, Value::Scalar(390.0));
    assert_eq!(tile.prop("mirrorEdges").unwrap().value, Value::Bool(true));
    let time = 0.5 + 5910.0 / 25600.0;
    let tiled = s.render(cid, Tick::from_seconds_f64(time), Default::default());
    let saved = s.project.to_file_json().unwrap();
    disable(&mut s, "ec.stylize.motiontile");
    let untiled = s.render(cid, Tick::from_seconds_f64(time), Default::default());
    let covered = |image: &effectcraft_raster::Image| image.data.iter().filter(|p| p[3] > 0.5).count();
    assert!(covered(&tiled) > covered(&untiled) + 1000, "Motion Tile must expand actual content before Scale");
    println!("Combined preset covers {} pixels with Motion Tile, {} without", covered(&tiled), covered(&untiled));
    s.project = Arc::new(Project::from_json(&saved).unwrap());
    persistence_export_and_report(&mut s, cid, layer, time, "motion-tile");
    assert_eq!(std::fs::read(path).unwrap(), original);
}
