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
