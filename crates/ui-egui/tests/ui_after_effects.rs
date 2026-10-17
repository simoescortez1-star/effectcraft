//! Native plugin management stays command-driven and never promotes a file to a renderable effect.
use effectcraft_engine::Session;
use effectcraft_ui_egui::{Dialog, EffectcraftApp};
use egui_kittest::Harness;
use serde_json::json;

#[test]
fn aex_manager_exposes_actions_and_keeps_discovery_separate_from_execution() {
    let dir = std::env::temp_dir().join(format!("effectcraft-aex-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.aex"), b"invalid").unwrap();
    let mut session = Session::default();
    session.execute("effect.aex.addFolder", json!({"folder": dir})).unwrap();
    session.execute("effect.aex.scan", json!({})).unwrap();
    session.execute("effect.aex.manager", json!({})).unwrap();
    let mut harness = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_| EffectcraftApp::new(session));
    harness.run_steps(3);
    assert_eq!(harness.state().dialog, Some(Dialog::PluginManager));
    for id in [
        "aex.folder",
        "aex.addFolder",
        "aex.scan",
        "aex.installed",
        "aex.bundled",
        "aex.plugin.0.enabled",
        "aex.plugin.0.load",
        "aex.plugin.0.apply",
        "aex.close",
    ] {
        assert!(harness.state().auto.find(id).is_some(), "missing automation id {id}");
    }
    let plugin = &harness.state().session.ae_plugins.plugins[0];
    assert!(!plugin.loaded && !plugin.initialized && !plugin.rendered && !plugin.fidelity_verified);
    assert!(!plugin.can_load(), "invalid DLLs must not offer native execution");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ffx_report_exports_preserved_json_through_the_real_button_and_picker_hook() {
    use std::sync::Arc;
    let path = std::env::temp_dir().join(format!("cortez-ffx-ui-report-{}.json", std::process::id()));
    let mut s = Session::default();
    s.execute("comp.new", json!({"width":32,"height":32})).unwrap();
    let layer = s.execute("layer.newSolid", json!({"color":"#ffffff"})).unwrap()["layer"].as_u64().unwrap();
    let cid = s.active_comp_id().unwrap();
    // This exercises the report UI contract, not binary FFX decoding.
    let record = json!({"source":"original-report-contract","preset":{"effectcraftPreset":1,"nodes":[],"sourceHex":"cafe"},"conversionReport":{"report":{"warnings":[]}}});
    Arc::make_mut(&mut s.project).comp_mut(cid).unwrap().layers[0].comment = format!("\nEffectCraft FFX preservation:\n{record}");
    let mut app = EffectcraftApp::new(s);
    let selected = path.to_string_lossy().into_owned();
    app.hooks.pick_save_file = Some(Box::new(move |_, _| Some(selected.clone())));
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_| app);
    h.run_steps(3);
    let ctx = h.ctx.clone();
    h.state_mut().session.events.push(effectcraft_engine::Event::Frontend {command:"app.showReport".into(),params:json!({"title":"After Effects Preset Report","summary":"Applied a mapped property; visual equivalence unverified.","text":"Preserved source values and diagnostics","json":{"layers":[layer]}})});
    h.run_steps(3);
    let e = h.state().auto.find("report.exportJson").unwrap();
    let at = egui::pos2(e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.input_mut().events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
    h.step();
    h.input_mut().events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
    h.run_steps(2);
    let exported: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(exported["kind"], "CortezEffectsFFXReport");
    assert_eq!(exported["record"], record);
    std::fs::remove_file(path).unwrap();
    effectcraft_ui_egui::menus::invoke(h.state_mut(), &ctx, "app.gpuInfo", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.state().auto.find("report.exportJson").is_none(), "ordinary information dialogs must not export a stale FFX report");
}
