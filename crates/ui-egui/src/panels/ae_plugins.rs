//! Native-plugin inventory. Discovery and availability do not imply runtime support.
use crate::{
    EffectcraftApp,
    i18n::{tr, tr_args},
    theme::Tokens,
};
use egui::vec2;
use serde_json::json;

pub fn show(app: &mut EffectcraftApp, ctx: &egui::Context, tokens: &Tokens) {
    let runtime = match app.session.execute("effect.aex.list", json!({})) {
        Ok(list) => list["runtime"].clone(),
        Err(error) => {
            app.ui.status = error.to_string();
            serde_json::Value::Null
        }
    };
    let platform = runtime["platformSupported"].as_bool().unwrap_or(false);
    let worker = runtime["workerFound"].as_bool().unwrap_or(false);
    let bundled = runtime["bundledTestFolder"].as_str();
    let folders = app.session.prefs.after_effects.folders.clone();
    let scan = app.session.ae_plugins.clone();
    let mut actions = vec![];
    let mut close = false;
    super::dialogs::modal_fit(ctx, "Plugin Manager", vec2(800.0, 580.0), tokens, |ui, height| {
        let (mut body, body_height) = super::dialogs::above_footer(ui, height, |ui| {
            let r = ui.button(tr("Close"));
            app.auto.add("aex.close", r.rect, "Close");
            close = r.clicked();
        });
        body.vertical(|ui| {
            ui.label(tr("After Effects native plugins — experimental CPU runtime"));
            ui.label(tr("Scanning does not execute plugins. Loading runs a separate worker: CPU Smart Render, float sliders, 32-bit frames. Commercial compatibility and visual fidelity are unverified."));
            ui.label(tr(if !platform {"Native AEX requires Windows x64."} else if worker {"Native worker found; loading is explicit."} else {"Native worker missing. Extract the entire Windows ZIP."}));
            ui.horizontal(|ui| {
                let r = ui.text_edit_singleline(&mut app.dialog_state.ae_folder);
                app.auto.add_response("aex.folder", &r, "Plugin folder (absolute path)");
                let r = ui.button(tr("Add folder"));
                app.auto.add("aex.addFolder", r.rect, "Add folder");
                if r.clicked() { actions.push(("effect.aex.addFolder", json!({"folder": app.dialog_state.ae_folder}))); }
            });
            ui.horizontal(|ui| {
                for (id, label, params) in [("aex.scan", "Rescan folders", json!({})), ("aex.installed", "Find installed plugins", json!({"installed": true}))] {
                    let r = ui.button(tr(label));
                    app.auto.add(id, r.rect, label);
                    if r.clicked() { actions.push(("effect.aex.scan", params)); }
                }
                let r=ui.add_enabled(bundled.is_some(),egui::Button::new(tr("Scan bundled test plugin")));
                app.auto.add("aex.bundled",r.rect,"Scan the redistributable bundled test plugin without loading it");
                if r.clicked() && let Some(folder)=bundled {
                    actions.push(("effect.aex.addFolder",json!({"folder":folder})));
                    actions.push(("effect.aex.scan",json!({})));
                }
            });
            egui::ScrollArea::vertical().max_height((body_height - 190.0).max(40.0)).show(ui, |ui| {
                for (i, folder) in folders.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(folder);
                        let r = ui.button(tr("Remove"));
                        app.auto.add(&format!("aex.folder.{i}.remove"), r.rect, "Remove folder");
                        if r.clicked() { actions.push(("effect.aex.addFolder", json!({"folder": folder, "remove": true}))); }
                    });
                }
                ui.separator();
                if scan.plugins.is_empty() { ui.label(tr("No candidates found. Add a folder and scan to inspect plugins.")); }
                for (i, plugin) in scan.plugins.iter().enumerate() {
                    ui.group(|ui| {
                        let mut enabled = plugin.enabled;
                        let r = ui.checkbox(&mut enabled, &plugin.path);
                        app.auto.add(&format!("aex.plugin.{i}.enabled"), r.rect, "Enable native plugin");
                        if r.changed() { actions.push(("effect.aex.enable", json!({"path": plugin.path, "enabled": enabled}))); }
                        ui.label(format!("{:?}: {}", plugin.compatibility, plugin.diagnostic));
                        ui.label(tr_args("Plugin: {}",&[&std::path::Path::new(&plugin.path).file_stem().map(|s|s.to_string_lossy()).unwrap_or_default()]));
                        ui.label(tr_args("Loaded: {} · Initialized: {} · Parameters: {} · Rendered: {} · Visual fidelity: unverified",&[&plugin.loaded,&plugin.initialized,&plugin.parameters,&plugin.rendered]));
                        let executable=plugin.can_load() && platform && worker;
                        ui.horizontal(|ui| {
                            let r=ui.add_enabled(executable,egui::Button::new(tr("Load and test")));
                            app.auto.add(&format!("aex.plugin.{i}.load"),r.rect,"Execute plugin in external worker and test a frame");
                            if r.clicked() { actions.push(("effect.aex.load",json!({"path":plugin.path}))); }
                            let r=ui.add_enabled(executable && !app.session.state.selected_layers.is_empty(),egui::Button::new(tr("Load, test and apply")));
                            app.auto.add(&format!("aex.plugin.{i}.apply"),r.rect,"Load/test the native effect and add it to selected layers");
                            if r.clicked() { actions.push(("effect.aex.loadAndApply",json!({"path":plugin.path}))); }
                        });
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
