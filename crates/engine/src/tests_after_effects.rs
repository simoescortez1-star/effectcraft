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
