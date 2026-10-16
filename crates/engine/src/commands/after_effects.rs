//! Passive native-plugin management; never register untested DLLs as renderable effects.
use super::{CommandSpec, always, bad, str_p};
use crate::{Result, Session, cmd};
use effectcraft_ae_bridge::discovery;
use serde_json::{Value, json};

fn change_folders(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "folder").ok_or_else(|| bad("effect.aex.addFolder", "missing folder"))?;
    if path.len() > 4096 || !std::path::Path::new(path).is_absolute() {
        return Err(bad("effect.aex.addFolder", "folder must be an absolute path of at most 4096 bytes"));
    }
    let remove = p.get("remove").map(|v| v.as_bool().ok_or_else(|| bad("effect.aex.addFolder", "remove must be a boolean"))).transpose()?.unwrap_or(false);
    let config = &mut s.prefs.after_effects;
    if remove {
        config.folders.retain(|f| f != path);
    } else if !config.folders.iter().any(|f| f == path) {
        if config.folders.len() >= 64 {
            return Err(bad("effect.aex.addFolder", "64 folder limit reached"));
        }
        config.folders.push(path.into());
    }
    s.save_prefs();
    s.ae_plugins = Default::default();
    Ok(json!({"folders": s.prefs.after_effects.folders}))
}

fn scan(s: &mut Session, p: &Value) -> Result<Value> {
    let installed = p.get("installed").map(|v| v.as_bool().ok_or_else(|| bad("effect.aex.scan", "installed must be a boolean"))).transpose()?.unwrap_or(false);
    let mut config = s.prefs.after_effects.clone();
    if installed {
        for folder in discovery::default_folders() {
            if !config.folders.contains(&folder) {
                config.folders.push(folder);
            }
        }
    }
    s.ae_plugins = discovery::scan(&config);
    Ok(json!(s.ae_plugins))
}

fn toggle(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad("effect.aex.enable", "missing path"))?;
    let enabled = p.get("enabled").and_then(Value::as_bool).ok_or_else(|| bad("effect.aex.enable", "missing enabled boolean"))?;
    // Windows canonical paths use an extended prefix; clients may supply normal DOS paths.
    let canonical = std::fs::canonicalize(path).ok();
    let entry = s
        .ae_plugins
        .plugins
        .iter_mut()
        .find(|e| e.path == path || canonical.as_ref().is_some_and(|p| std::path::Path::new(&e.path) == p))
        .ok_or_else(|| bad("effect.aex.enable", "scan the plugin before changing its state"))?;
    let path = entry.path.clone();
    if !enabled && !s.prefs.after_effects.disabled.iter().any(|x| x == &path) && s.prefs.after_effects.disabled.len() >= 20_000 {
        return Err(bad("effect.aex.enable", "disabled plugin limit reached"));
    }
    entry.enabled = enabled;
    s.prefs.after_effects.disabled.retain(|x| x != &path);
    if !enabled {
        s.prefs.after_effects.disabled.push(path.clone());
    }
    s.save_prefs();
    Ok(json!({"path": path, "enabled": enabled, "loaded": false}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("effect.aex.manager", "Plugin Manager...", ["Effect"], None, "{} — open passive AEX plugin manager", always, |s, _| {
            s.events.push(crate::Event::Frontend { command: "effect.aex.manager".into(), params: json!({}) });
            Ok(Value::Null)
        }),
        cmd!("effect.aex.addFolder", "Add Plugin Folder...", [], None, "{folder, remove? (bool)} — persist scan directories", always, change_folders),
        cmd!("effect.aex.scan", "Scan Native Plugins", [], None, "{installed? (bool)} — inspect files without executing DLLs", always, scan),
        cmd!("effect.aex.list", "List Native Plugins", [], None, "{} — passive candidates, stage results and diagnostics", always, |s, _| Ok(
            json!({"folders": s.prefs.after_effects.folders, "scan": s.ae_plugins})
        )),
        cmd!("effect.aex.enable", "Enable Native Plugin", [], None, "{path, enabled (bool)} — availability preference only, never loads code", always, toggle),
        cmd!(
            "anim.applyAfterEffectsPreset",
            "Apply After Effects Preset...",
            ["Animation"],
            None,
            "{path} — FFX decoding currently unsupported; fails without editing layers",
            always,
            |_, _| Err(bad(
                "anim.applyAfterEffectsPreset",
                "FFX decoding is not implemented: no verified format mapping or authorized reference corpus is available. No preset was applied; the source file is unchanged."
            ))
        ),
    ]
}
