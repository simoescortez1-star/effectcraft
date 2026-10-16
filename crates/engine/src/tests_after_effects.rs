//! Bridge contract tests are separate from the opt-in real binary AEP integration test.
use crate::{Services, Session};
use effectcraft_ae_bridge::{Conversion, ProjectConverter};
use effectcraft_project::{Expression, ItemId, Project};
use serde_json::json;
use std::sync::Arc;

struct Memory;
impl Services for Memory {
    fn read_file(&self, _: &str) -> std::io::Result<Vec<u8>> {
        Ok(b"opaque source, not an AEP fixture".to_vec())
    }
    fn write_file(&self, _: &str, _: &[u8]) -> std::io::Result<()> {
        Err(std::io::ErrorKind::PermissionDenied.into())
    }
}
struct Mock {
    project: String,
    report: String,
}
impl ProjectConverter for Mock {
    fn convert(&self, source: &[u8]) -> Result<Conversion, String> {
        assert_eq!(source, b"opaque source, not an AEP fixture");
        Ok(Conversion { project_json: self.project.clone(), report_json: self.report.clone() })
    }
}
fn report() -> String {
    json!({"report": {"compositions": 1, "layers": 1, "warnings": [{"context": "Main/Glow", "message": "unknown effect retained only in original source"}]}})
        .to_string()
}
fn converted_project() -> String {
    let mut s = Session::default();
    s.execute("comp.new", json!({"name": "Main", "width": 32, "height": 32, "duration": 2, "frameRate": 30})).unwrap();
    s.execute("layer.newSolid", json!({"name": "Red", "color": "#ff0000"})).unwrap();
    let cid = s.active_comp_id().unwrap();
    let project = Arc::make_mut(&mut s.project);
    let opacity = project.comp_mut(cid).unwrap().layers[0].props.prop_mut("transform/opacity").unwrap();
    opacity.expr = Some(Expression { text: "throw new Error('must not execute')".into(), enabled: true });
    opacity.keys.push(effectcraft_keyframe::Keyframe::new(effectcraft_time::Tick::ZERO, effectcraft_keyframe::Value::Scalar(100.0)));
    project.to_file_json().unwrap()
}

#[test]
fn aep_bridge_import_undo_redo_persistence_render_and_expression_isolation() {
    let mut s =
        Session { services: Arc::new(Memory), ae_converter: Some(Arc::new(Mock { project: converted_project(), report: report() })), ..Default::default() };
    let before = s.project.to_json();
    let result = s.execute("file.importAfterEffects", json!({"path": "Folder/Example.AEP"})).unwrap();
    assert!(result["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("unknown effect")));
    let folder = ItemId(result["folder"].as_u64().unwrap());
    let mut expected_report: serde_json::Value = serde_json::from_str(&report()).unwrap();
    expected_report["originalSource"] = json!("Folder/Example.AEP");
    assert_eq!(s.project.item(folder).unwrap().comment, expected_report.to_string());
    let cid = ItemId(result["primaryComp"].as_u64().unwrap());
    let comp = s.project.comp(cid).unwrap();
    let prop = comp.layers[0].props.prop("transform/opacity").unwrap();
    assert!(!prop.expr.as_ref().unwrap().enabled);
    assert_eq!(prop.keys.len(), 1);
    let frame = effectcraft_render::Renderer::new(&s.project, s.footage.as_ref(), Default::default()).comp_frame(cid, effectcraft_time::Tick::ZERO);
    assert_eq!((frame.width, frame.height), (32, 32));
    assert!(frame.data.iter().any(|p| p[0] > 0.9 && p[1] < 0.1 && p[3] > 0.9));
    let saved = s.project.to_file_json().unwrap();
    let reopened = Project::from_json(&saved).unwrap();
    assert_eq!(reopened.item(folder).unwrap().comment, expected_report.to_string());
    assert_eq!(reopened.comp(cid).unwrap().layers[0].props.prop("transform/opacity").unwrap().keys.len(), 1);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.project.to_json(), before);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(s.project.to_file_json().unwrap(), saved);
}

#[test]
fn unsupported_formats_bad_conversion_and_missing_worker_leave_document_intact() {
    let mut s = Session { services: Arc::new(Memory), ..Default::default() };
    s.execute("comp.new", json!({"name": "Work"})).unwrap();
    let before = s.project.to_json();
    for (command, path) in [("file.importAfterEffects", "x.aep"), ("file.importAfterEffects", "x.aepx"), ("anim.applyAfterEffectsPreset", "x.ffx")] {
        assert!(s.execute(command, json!({"path": path})).is_err());
        assert_eq!(s.project.to_json(), before);
    }
    for (project, report) in [("bad project".into(), report()), (converted_project(), "{}".into())] {
        s.ae_converter = Some(Arc::new(Mock { project, report }));
        assert!(s.execute("file.importAfterEffects", json!({"path": "x.aep"})).is_err());
        assert_eq!(s.project.to_json(), before);
    }
}

#[test]
fn aex_folders_persist_and_state_is_never_confused_with_compatibility() {
    let mut s = Session::default();
    assert!(s.execute("effect.aex.addFolder", json!({"folder": "relative"})).is_err());
    let dir = std::env::temp_dir().join(format!("effectcraft-aex-config-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let plugin = dir.join("bad.aex");
    std::fs::write(&plugin, b"this is not a DLL").unwrap();
    assert!(s.execute("effect.aex.addFolder", json!({"folder": dir, "remove": "yes"})).is_err());
    assert!(s.execute("effect.aex.scan", json!({"installed": 1})).is_err());
    s.execute("effect.aex.addFolder", json!({"folder": dir})).unwrap();
    let scan = s.execute("effect.aex.scan", json!({})).unwrap();
    assert_eq!(scan["plugins"][0]["compatibility"], "unsupported");
    assert_eq!(scan["plugins"][0]["rendered"], false);
    s.execute("effect.aex.enable", json!({"path": dir.join(".").join("bad.aex"), "enabled": false})).unwrap();
    let prefs = crate::prefs::Prefs::from_json(&s.prefs.to_json());
    assert_eq!(prefs.after_effects.disabled, vec![scan["plugins"][0]["path"].as_str().unwrap().to_string()]);
    let scan = s.execute("effect.aex.scan", json!({})).unwrap();
    assert_eq!(scan["plugins"][0]["enabled"], false);
    assert_eq!(scan["plugins"][0]["loaded"], false);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "requires pinned external converter and upstream synthetic AEP; see compatibility docs"]
fn aep_external_binary_end_to_end() {
    let executable = std::env::var_os("EFFECTCRAFT_AEP_CONVERTER").expect("explicit converter");
    let path = std::env::var("EFFECTCRAFT_TEST_AEP").expect("explicit synthetic test AEP");
    let original = std::fs::read(&path).unwrap();
    let mut s = Session {
        ae_converter: Some(Arc::new(effectcraft_ae_bridge::process::ExternalConverter {
            executable: executable.into(),
            timeout: std::time::Duration::from_secs(30),
        })),
        ..Default::default()
    };
    let imported = s.execute("file.importAfterEffects", json!({"path": path})).unwrap();
    let cid = ItemId(imported["primaryComp"].as_u64().unwrap());
    let comp = s.project.comp(cid).unwrap();
    assert_eq!((comp.width, comp.height), (1920, 1080));
    assert_eq!(comp.frame_rate.as_f64(), 30.0);
    let prop = comp.layers[0].props.prop("transform/opacity").unwrap();
    assert_eq!(prop.keys.len(), 1);
    assert_eq!(prop.keys[0].time, effectcraft_time::Tick::from_seconds_f64(0.5));
    assert_eq!(prop.keys[0].value, effectcraft_keyframe::Value::Scalar(80.0));
    assert!(!prop.expr.as_ref().unwrap().enabled);
    let saved = s.project.to_file_json().unwrap();
    assert_eq!(Project::from_json(&saved).unwrap().comp(cid).unwrap().layers.len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(imported["conversionReport"]["report"]["warnings"].is_array());
    assert_eq!(imported["conversionReport"]["originalSource"], path);
}

fn preset_target() -> (Session, u64) {
    let mut s = Session::default();
    Arc::make_mut(&mut s.project).settings.bit_depth = effectcraft_project::BitDepth::Bpc32;
    s.execute("comp.new", json!({"name":"Preset regression","width":32,"height":32,"duration":5,"frameRate":30})).unwrap();
    let layer = s.execute("layer.newSolid", json!({"name":"Target","color":"#ffffff"})).unwrap()["layer"].as_u64().unwrap();
    (s, layer)
}

#[test]
fn ffx_carrier_applies_at_current_time_with_undo_and_disabled_expressions() {
    let mut carrier: Project = Project::from_json(&converted_project()).unwrap();
    for item in carrier.items.values_mut() {
        if let effectcraft_project::ItemKind::Comp(comp) = &mut item.kind {
            Arc::make_mut(comp).layers[0].comment =
                json!({"effectcraftPreset":1,"mappedPaths":["transform/opacity"],"unavailableEffects":[],"sourceHex":"cafe"}).to_string();
        }
    }
    let (mut s, layer) = preset_target();
    s.services = Arc::new(Memory);
    s.ae_converter = Some(Arc::new(Mock { project: carrier.to_file_json().unwrap(), report: report() }));
    s.execute("time.set", json!({"time":1.0})).unwrap();
    let before = s.project.to_json();
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":"mock.ffx","layers":[layer]})).unwrap();
    assert_eq!(result["applied"], true);
    let l = &s.active_comp().unwrap().layers[0];
    let p = l.props.prop("transform/opacity").unwrap();
    assert_eq!(p.keys[0].time, effectcraft_time::Tick::from_seconds_f64(1.0));
    assert!(!p.expr.as_ref().unwrap().enabled);
    assert!(l.comment.contains("cafe"));
    let saved = s.project.to_file_json().unwrap();
    assert_eq!(Project::from_json(&saved).unwrap().comp(s.active_comp_id().unwrap()).unwrap().layers[0].comment, l.comment);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.project.to_json(), before);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(s.project.to_file_json().unwrap(), saved);
    let before = s.project.to_json();
    assert!(s.execute("anim.applyAfterEffectsPreset", json!({"path":"mock.ffx","layers":[999999]})).is_err());
    assert_eq!(s.project.to_json(), before);
}

#[test]
fn ffx_effect_uid_exhaustion_is_an_error_without_a_partial_edit() {
    let mut carrier = Project::from_json(&converted_project()).unwrap();
    let mut next = carrier.next_id;
    let spec = effectcraft_effects::lookup("ec.blur.gaussian").unwrap();
    let effect = effectcraft_effects::instantiate(spec, &mut effectcraft_project::build::Ids(&mut next), "Gaussian", [32.0, 32.0]);
    let cid = *carrier.comps().next().unwrap().0;
    let layer = &mut carrier.comp_mut(cid).unwrap().layers[0];
    layer.props.sub_mut("effects").unwrap().children.push(effect.into());
    layer.comment = json!({"effectcraftPreset":1,"mappedPaths":[],"unavailableEffects":[]}).to_string();
    carrier.next_id = next;
    let (mut s, layer) = preset_target();
    s.services = Arc::new(Memory);
    s.ae_converter = Some(Arc::new(Mock { project: carrier.to_file_json().unwrap(), report: report() }));
    Arc::make_mut(&mut s.project).next_id = u64::MAX;
    let before = s.project.to_json();
    let error = s.execute("anim.applyAfterEffectsPreset", json!({"path":"mock.ffx","layers":[layer]})).unwrap_err();
    assert!(error.to_string().contains("not enough IDs"), "{error}");
    assert_eq!(s.project.to_json(), before);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "requires edition converter and original fixtures generated by packaging/after-effects/generate_fixtures.py"]
fn ffx_external_generated_end_to_end() {
    let path = std::env::var("EFFECTCRAFT_TEST_FFX").unwrap();
    let executable = std::env::var_os("EFFECTCRAFT_AEP_CONVERTER").unwrap();
    let original = std::fs::read(&path).unwrap();
    let (mut s, layer) = preset_target();
    s.ae_converter =
        Some(Arc::new(effectcraft_ae_bridge::process::ExternalConverter { executable: executable.into(), timeout: std::time::Duration::from_secs(30) }));
    s.execute("time.set", json!({"time":0.5})).unwrap();
    let before = s.project.to_json();
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":path,"layers":[layer]})).unwrap();
    assert_eq!(result["applied"], true);
    let p = s.active_comp().unwrap().layers[0].props.prop("transform/opacity").unwrap();
    assert_eq!(p.keys.len(), 2);
    assert_eq!(p.keys[0].time, effectcraft_time::Tick::from_seconds_f64(0.5));
    assert_eq!(p.keys[1].time, effectcraft_time::Tick::from_seconds_f64(1.5));
    assert_eq!(p.keys[0].out_interp, effectcraft_keyframe::Interp::Bezier);
    let cid = s.active_comp_id().unwrap();
    let frame = |seconds| s.render(cid, effectcraft_time::Tick::from_seconds_f64(seconds), Default::default()).data[0][3];
    assert!(frame(0.5) < 0.01);
    assert!(frame(1.5) > 0.99);
    let middle = frame(0.75);
    assert!(middle > 0.1 && middle < 0.2, "saved Bezier differs from linear: {middle}");
    let saved = s.project.to_file_json().unwrap();
    Project::from_json(&saved).unwrap();
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.project.to_json(), before);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(s.project.to_file_json().unwrap(), saved);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let unavailable = std::path::Path::new(&path).with_file_name("unavailable.ffx");
    let before = s.render(cid, effectcraft_time::Tick::from_seconds_f64(1.5), Default::default());
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":unavailable,"layers":[layer]})).unwrap();
    assert_eq!(result["applied"], false);
    assert_eq!(result["preserved"], true);
    assert_eq!(result["unavailableEffects"][0], "org.effectcraft.test.unavailable");
    assert_eq!(s.render(cid, effectcraft_time::Tick::from_seconds_f64(1.5), Default::default()).data, before.data);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "requires external converter and original generated direct-tdbs Scale/transform fixtures"]
fn ffx_direct_transform_presets_render_and_preserve_saved_timebase() {
    let path = std::path::PathBuf::from(std::env::var_os("EFFECTCRAFT_TEST_FFX").unwrap()).with_file_name("scale.ffx");
    let (mut s, layer) = check_scale_preset(&path, 48000, [24000, 48000], [[50., 75., 100.], [150., 125., 100.]], [65.625, 82.8125, 100.]);
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":path.with_file_name("transforms.ffx"),"layers":[layer]})).unwrap();
    assert_eq!(result["propertiesApplied"], 4);
    assert_eq!(result["keyframesApplied"], 8);
    for (path, value) in [
        ("transform/position", json!([16., 16., 0.])),
        ("transform/anchor", json!([16., 16., 0.])),
        ("transform/rotation", json!(20.)),
        ("transform/opacity", json!(60.)),
    ] {
        let prop = s.active_comp().unwrap().layers[0].props.prop(path).unwrap();
        assert_eq!(prop.keys.len(), 2);
        assert_eq!(serde_json::to_value(prop.keys[0].value.clone()).unwrap()["v"], value);
    }
    let before = s.project.to_json();
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":path.with_file_name("conflicting-clock.ffx"),"layers":[layer]}));
    assert!(result.is_err(), "conflicting timebases must not silently change key timing");
    assert_eq!(s.project.to_json(), before);
}

#[cfg(not(target_arch = "wasm32"))]
fn check_scale_preset(path: &std::path::Path, clock: i64, ticks: [i64; 2], values: [[f64; 3]; 2], quarter: [f64; 3]) -> (Session, u64) {
    use effectcraft_time::{TICKS_PER_SECOND, Tick};
    let executable = std::env::var_os("EFFECTCRAFT_AEP_CONVERTER").unwrap();
    let original = std::fs::read(path).unwrap();
    let make_target = || {
        let mut s = Session::default();
        Arc::make_mut(&mut s.project).settings.bit_depth = effectcraft_project::BitDepth::Bpc32;
        s.execute("comp.new", json!({"name":"FFX Scale","width":32,"height":32,"duration":5,"frameRate":60})).unwrap();
        let layer = s.execute("layer.newSolid", json!({"width":8,"height":8,"color":"#ffffff"})).unwrap()["layer"].as_u64().unwrap();
        (s, layer)
    };
    let (mut s, layer) = make_target();
    s.ae_converter =
        Some(Arc::new(effectcraft_ae_bridge::process::ExternalConverter { executable: executable.into(), timeout: std::time::Duration::from_secs(30) }));
    s.execute("time.set", json!({"time":0.5})).unwrap();
    let before = s.project.to_json();
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":path,"layers":[layer]})).unwrap();
    assert_eq!(result["applied"], true);
    assert_eq!(result["propertiesApplied"], 1);
    assert_eq!(result["keyframesApplied"], 2);
    assert_eq!(result["sourceTicksPerSecond"], clock);
    assert_eq!(result["unavailableEffects"], json!([]));
    let prop = s.active_comp().unwrap().layers[0].props.prop("transform/scale").unwrap();
    let uid = prop.uid;
    assert_eq!(prop.keys.len(), 2);
    let times = ticks.map(|time| Tick(TICKS_PER_SECOND / 2 + TICKS_PER_SECOND * time / clock));
    for index in 0..2 {
        assert_eq!(prop.keys[index].time, times[index]);
        for (actual, expected) in prop.keys[index].value.as_vec3().iter().zip(values[index]) {
            assert!((actual - expected).abs() < 1e-8);
        }
        assert_eq!(prop.keys[index].in_interp, effectcraft_keyframe::Interp::Bezier);
        assert_eq!(prop.keys[index].out_interp, effectcraft_keyframe::Interp::Bezier);
    }
    let quarter_time = Tick(times[0].0 + (times[1].0 - times[0].0) / 4);
    for (actual, expected) in prop.value_at(quarter_time).as_vec3().iter().zip(quarter) {
        assert!((actual - expected).abs() < 1e-6, "Bezier value: {actual} != {expected}");
    }
    assert!(s.events.iter().any(|event| matches!(event, crate::Event::Frontend{command,params} if command=="timeline.revealProps" && params["props"].as_array().unwrap().iter().any(|p| p["prop"]==uid))));
    let cid = s.active_comp_id().unwrap();
    let (mut reference, reference_layer) = make_target();
    let reference_cid = reference.active_comp_id().unwrap();
    let mut frames = Vec::new();
    for (time, scale) in [(times[0], values[0]), (quarter_time, quarter), (times[1], values[1])] {
        reference.execute("prop.set", json!({"layer":reference_layer,"path":"transform/scale","value":scale})).unwrap();
        let expected = reference.render(reference_cid, time, Default::default());
        let rendered = s.render(cid, time, Default::default());
        for (actual, expected) in rendered.data.iter().flatten().zip(expected.data.iter().flatten()) {
            assert!((actual - expected).abs() < 0.0001, "rendered pixel differs from explicit expected scale");
        }
        frames.push(rendered.data);
    }
    assert_ne!(frames[0], frames[1], "advancing frames must visibly animate Scale");
    assert_ne!(frames[0], frames[2]);
    let saved = s.project.to_file_json().unwrap();
    let reopened = Session { project: Arc::new(Project::from_json(&saved).unwrap()), ..Default::default() };
    assert_eq!(reopened.render(cid, times[1], Default::default()).data, frames[2]);
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.project.to_json(), before);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(s.project.to_file_json().unwrap(), saved);
    assert_eq!(std::fs::read(path).unwrap(), original);
    (s, layer)
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "private uploaded scale.ffx is never redistributed; set EFFECTCRAFT_TEST_USER_SCALE explicitly"]
fn user_scale_ffx_creates_saved_keys_and_renders_animated_pixels() {
    let path = std::path::PathBuf::from(std::env::var_os("EFFECTCRAFT_TEST_USER_SCALE").unwrap());
    // Independently solve the saved temporal Bezier at one-quarter of the segment.
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for _ in 0..80 {
        let u = (low + high) / 2.0;
        let x = 3.0 * (1.0 - u).powi(2) * u * 0.91 + 3.0 * (1.0 - u) * u * u * 0.999 + u * u * u;
        if x < 0.25 {
            low = u;
        } else {
            high = u;
        }
    }
    let u = (low + high) / 2.0;
    let last_handle = 380.0 - 10416.0 * 100.0 * 0.25 * 0.001;
    let x = (1.0 - u).powi(3) * 100.0 + 3.0 * (1.0 - u).powi(2) * u * 100.0 + 3.0 * (1.0 - u) * u * u * last_handle + u * u * u * 380.0;
    check_scale_preset(&path, 30720, [19456, 27136], [[100., 100., 100.], [380., 100., 100.]], [x, 100., 100.]);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "private user fixtures are never redistributed; set EFFECTCRAFT_TEST_USER_AEP and EFFECTCRAFT_TEST_USER_FFX explicitly"]
fn user_after_effects_inputs_preserve_measured_structure_and_unavailable_effect() {
    let aep = std::env::var("EFFECTCRAFT_TEST_USER_AEP").unwrap();
    let ffx = std::env::var("EFFECTCRAFT_TEST_USER_FFX").unwrap();
    let executable = std::env::var_os("EFFECTCRAFT_AEP_CONVERTER").unwrap();
    let original_aep = std::fs::read(&aep).unwrap();
    let original_ffx = std::fs::read(&ffx).unwrap();
    let mut s = Session {
        ae_converter: Some(Arc::new(effectcraft_ae_bridge::process::ExternalConverter {
            executable: executable.into(),
            timeout: std::time::Duration::from_secs(30),
        })),
        ..Default::default()
    };
    let imported = s.execute("file.importAfterEffects", json!({"path":aep})).unwrap();
    assert_eq!(imported["comps"].as_array().unwrap().len(), 2);
    assert_eq!(imported["missingMedia"].as_array().unwrap().len(), 1);
    let comps: Vec<_> = s.project.comps().collect();
    assert_eq!(comps.len(), 2);
    for (_, comp) in &comps {
        assert_eq!((comp.width, comp.height), (1440, 1440));
        assert_eq!(comp.frame_rate.as_f64(), 60.0);
        assert_eq!(comp.layers.len(), 1);
    }
    let p = comps.iter().flat_map(|(_, c)| &c.layers).filter_map(|l| l.props.prop("transform/scale")).find(|p| p.keys.len() == 2).unwrap();
    assert_eq!(p.keys[1].time, effectcraft_time::Tick(effectcraft_time::TICKS_PER_SECOND * 59 / 60));
    assert_eq!(p.keys[0].out_interp, effectcraft_keyframe::Interp::Bezier);
    assert!((p.keys[1].value.as_vec3()[0] - 156.0).abs() < 1e-9);
    for item in s.project.items.values() {
        if let effectcraft_project::ItemKind::Footage(f) = &item.kind {
            assert!(!f.path.is_empty());
            assert!(!item.name.is_empty());
            assert!(f.missing);
            assert!(f.has_video);
        }
    }
    let cid = ItemId(imported["primaryComp"].as_u64().unwrap());
    let frame = s.render(cid, effectcraft_time::Tick::ZERO, Default::default());
    assert_eq!((frame.width, frame.height), (1440, 1440)); // Missing source: structural render, no fidelity claim.
    let layer = s.project.comp(cid).unwrap().layers[0].id.0;
    s.execute("comp.open", json!({"comp":cid.0})).unwrap();
    s.execute("layer.select", json!({"layers":[layer]})).unwrap();
    let before = frame.data;
    let result = s.execute("anim.applyAfterEffectsPreset", json!({"path":ffx,"layers":[layer]})).unwrap();
    assert_eq!(result["applied"], false);
    assert_eq!(result["preserved"], true);
    assert_eq!(result["unavailableEffects"], json!(["S_Shake"]));
    let comment = &s.project.comp(cid).unwrap().layers[0].comment;
    let record: serde_json::Value = serde_json::from_str(comment.split("EffectCraft FFX preservation:\n").last().unwrap()).unwrap();
    let nodes = record["preset"]["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    let parameters = nodes[0]["children"].as_array().unwrap();
    assert_eq!(parameters.len(), 84);
    let animated = parameters.iter().find(|p| p["match_name"] == "S_Shake-0050").unwrap();
    assert_eq!(animated["property"]["keyframes"].as_array().unwrap().len(), 2);
    assert_eq!(animated["property"]["keyframes"][1]["time_ticks"], 5600);
    let expected: String = original_ffx.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(record["preset"]["sourceHex"], expected);
    assert_eq!(s.render(cid, effectcraft_time::Tick::ZERO, Default::default()).data, before);
    Project::from_json(&s.project.to_file_json().unwrap()).unwrap();
    assert_eq!(std::fs::read(aep).unwrap(), original_aep);
    assert_eq!(std::fs::read(ffx).unwrap(), original_ffx);
}

#[test]
#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
#[ignore = "requires the optional Windows native worker and original Gain AEX fixture"]
fn aex_native_worker_renders_and_recovers_after_child_exit() {
    let path = std::path::PathBuf::from(std::env::var_os("EFFECTCRAFT_TEST_AEX").unwrap());
    let (mut s, layer) = preset_target();
    s.execute("effect.aex.addFolder", json!({"folder":path.parent().unwrap()})).unwrap();
    s.execute("effect.aex.scan", json!({})).unwrap();
    assert_eq!(s.execute("effect.aex.list", json!({})).unwrap()["runtime"]["workerFound"], true);
    let loaded = s.execute("effect.aex.loadAndApply", json!({"path":path,"layers":[layer]})).unwrap();
    assert_eq!(loaded["rendered"], true);
    assert_eq!(loaded["fidelityVerified"], false);
    let reloaded = s.execute("effect.aex.load", json!({"path":path})).unwrap();
    assert_eq!(reloaded["effect"], loaded["effect"]);
    assert_eq!(reloaded["rendered"], true);
    assert_eq!(reloaded["parameters"], 1);
    assert_eq!(s.active_comp().unwrap().layers[0].props.sub("effects").unwrap().groups().count(), 1);
    s.execute("prop.set", json!({"layer":layer,"path":"effects/#1/p1","value":0.5})).unwrap();
    let cid = s.active_comp_id().unwrap();
    let frame = s.render(cid, effectcraft_time::Tick::ZERO, Default::default());
    assert!((frame.data[0][0] - 0.5).abs() < 0.001);
    let later = s.render(cid, effectcraft_time::Tick::from_seconds_f64(1.0), Default::default());
    assert!((later.data[0][0] - 0.6).abs() < 0.001);
    let before = s.project.to_json();
    s.execute("prop.set", json!({"layer":layer,"path":"effects/#1/p1","value":4.0})).unwrap();
    let failed = s.render(cid, effectcraft_time::Tick::ZERO, Default::default());
    assert!((failed.data[0][0] - 1.0).abs() < 0.001, "failure preserves input pixels");
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.project.to_json(), before);
    let recovered = s.render(cid, effectcraft_time::Tick::from_seconds_f64(0.5), Default::default());
    assert!((recovered.data[0][0] - 0.55).abs() < 0.001);
    let saved = s.project.to_file_json().unwrap();
    Project::from_json(&saved).unwrap();
    s.execute("effect.aex.enable", json!({"path":path,"enabled":false})).unwrap();
    let disabled = s.render(cid, effectcraft_time::Tick::from_seconds_f64(1.2), Default::default());
    assert!((disabled.data[0][0] - 1.0).abs() < 0.001);
}
