//! Native-plugin inventory. Discovery and availability do not imply runtime support.
use crate::{EffectcraftApp, theme::Tokens};
use egui::vec2;
use serde_json::json;

pub fn show(app: &mut EffectcraftApp, ctx: &egui::Context, tokens: &Tokens) {
    if let Err(error) = app.session.execute("effect.aex.list", json!({})) {
        app.ui.status = error.to_string();
    }
    let folders = app.session.prefs.after_effects.folders.clone();
    let scan = app.session.ae_plugins.clone();
    let mut actions = vec![];
    let mut close = false;
    super::dialogs::modal_fit(ctx, "Plugin Manager", vec2(800.0, 580.0), tokens, |ui, height| {
        let (mut body, body_height) = super::dialogs::above_footer(ui, height, |ui| {
            let r = ui.button("Close");
            app.auto.add("aex.close", r.rect, "Close");
            close = r.clicked();
        });
        body.vertical(|ui| {
            ui.label("After Effects native plugins — experimental CPU runtime");
            ui.label("Scanning reads headers only. Load and test explicitly executes a plugin in a separate process. Supported subset: CPU Smart Render, float sliders, 32-bit frames. Visual fidelity and commercial plugins remain unverified.");
            ui.horizontal(|ui| {
                let r = ui.text_edit_singleline(&mut app.dialog_state.ae_folder);
                app.auto.add_response("aex.folder", &r, "Plugin folder (absolute path)");
                let r = ui.button("Add folder");
                app.auto.add("aex.addFolder", r.rect, "Add folder");
                if r.clicked() { actions.push(("effect.aex.addFolder", json!({"folder": app.dialog_state.ae_folder}))); }
            });
            ui.horizontal(|ui| {
                for (id, label, params) in [("aex.scan", "Rescan folders", json!({})), ("aex.installed", "Find installed plugins", json!({"installed": true}))] {
                    let r = ui.button(label);
                    app.auto.add(id, r.rect, label);
                    if r.clicked() { actions.push(("effect.aex.scan", params)); }
                }
            });
            egui::ScrollArea::vertical().max_height((body_height - 125.0).max(40.0)).show(ui, |ui| {
                for (i, folder) in folders.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(folder);
                        let r = ui.button("Remove");
                        app.auto.add(&format!("aex.folder.{i}.remove"), r.rect, "Remove folder");
                        if r.clicked() { actions.push(("effect.aex.addFolder", json!({"folder": folder, "remove": true}))); }
                    });
                }
                ui.separator();
                if scan.plugins.is_empty() { ui.label("No candidates found. Add a folder and scan to inspect plugins."); }
                for (i, plugin) in scan.plugins.iter().enumerate() {
                    ui.group(|ui| {
                        let mut enabled = plugin.enabled;
                        let r = ui.checkbox(&mut enabled, &plugin.path);
                        app.auto.add(&format!("aex.plugin.{i}.enabled"), r.rect, "Enable native plugin");
                        if r.changed() { actions.push(("effect.aex.enable", json!({"path": plugin.path, "enabled": enabled}))); }
                        ui.label(format!("{:?}: {}", plugin.compatibility, plugin.diagnostic));
                        ui.label(format!("Plugin: {} · Architecture: x64 candidate",std::path::Path::new(&plugin.path).file_stem().map(|s|s.to_string_lossy()).unwrap_or_default()));
                        ui.label(format!("Loaded: {} · Initialized: {} · Parameters: {} · Rendered: {} · Visual fidelity: unverified",plugin.loaded,plugin.initialized,plugin.parameters,plugin.rendered));
                        let r=ui.add_enabled(plugin.enabled && cfg!(target_os="windows"),egui::Button::new("Load and test"));
                        app.auto.add(&format!("aex.plugin.{i}.load"),r.rect,"Execute plugin in external worker and test a frame");
                        if r.clicked() { actions.push(("effect.aex.load",json!({"path":plugin.path}))); }
                    });
                }
                for diagnostic in &scan.diagnostics { ui.label(diagnostic); }
            });
        });
    });
    for (command, params) in actions {
        if let Err(e) = app.session.execute(command, params) {
            app.ui.status = e.to_string();
        }
    }
    if close {
        app.dialog = None;
    }
}
