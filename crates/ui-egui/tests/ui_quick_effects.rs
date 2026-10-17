//! Exercise real keyboard input, commands, pixels and persistent settings, not a mock console.
use std::sync::Arc;

use effectcraft_engine::Session;
use effectcraft_engine::config::{ConfigStore, MemoryConfig};
use effectcraft_ui_egui::{Dialog, EffectcraftApp, menus};
use egui::{Event, Key, Modifiers, pos2, vec2};
use egui_kittest::Harness;
use serde_json::{Value, json};

fn session() -> Session {
    let mut session = Session::default();
    session.prefs.startup.show_home_on_launch = false;
    session.prefs.general.language = "en".into();
    session.execute("comp.new", json!({"name":"Original Console Test","width":32,"height":24,"duration":3})).unwrap();
    session.execute("layer.newSolid", json!({"color":"#2080df"})).unwrap();
    session
}

fn harness(session: Session) -> Harness<'static, EffectcraftApp> {
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).build_eframe(|_| EffectcraftApp::new(session));
    h.run_steps(3);
    h
}

fn invoke(h: &mut Harness<'_, EffectcraftApp>, id: &str, params: Value) -> Result<Value, String> {
    let ctx = h.ctx.clone();
    menus::invoke(h.state_mut(), &ctx, id, params)
}

fn press(h: &mut Harness<'_, EffectcraftApp>, key: Key, modifiers: Modifiers) {
    h.input_mut().events.push(Event::ModifiersChanged(modifiers));
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.step();
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(3);
}

fn click(h: &mut Harness<'_, EffectcraftApp>, id: &str) {
    h.run_steps(4);
    let element = h.state().auto.find(id).unwrap_or_else(|| panic!("missing {id}"));
    let at = pos2(element.rect[0] + element.rect[2] * 0.5, element.rect[1] + element.rect[3] * 0.5);
    h.input_mut().events.push(Event::PointerMoved(at));
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

fn pixels(session: &mut Session) -> Vec<u8> {
    let cid = session.active_comp_id().unwrap();
    let background = session.active_comp().unwrap().background;
    session.render(cid, session.time(), Default::default()).to_rgba8_over(background)
}

#[test]
fn ctrl_space_search_enter_applies_real_pixels_and_one_undo_redo_from_timeline_and_viewer() {
    let mut h = harness(session());
    let before = h.state().session.project.clone();
    let original = pixels(&mut h.state_mut().session);
    for focus in ["Timeline", "Composition"] {
        invoke(&mut h, "window.panel", json!({"panel":focus})).unwrap();
        h.run_steps(2);
        let (mods, key) = menus::parse_shortcut("Ctrl+Space").unwrap();
        press(&mut h, key, mods);
        assert_eq!(h.state().dialog, Some(Dialog::QuickEffects));
        assert!(!h.state().playback.playing, "Ctrl+Space must not trigger Spacebar preview");
        assert!(h.ctx.memory(|m| m.has_focus(egui::Id::new("quick-effects-search"))), "search is focused on open");
        h.input_mut().events.push(Event::Text("tint".into()));
        h.run_steps(3);
        let result = invoke(&mut h, "quickEffects.results", json!({})).unwrap();
        assert_eq!(result["results"][0]["entry"]["id"], "ec.color.tint");
        press(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.state().dialog, None);
        assert!(h.state().session.active_comp().unwrap().layers[0].effects().unwrap().groups().any(|fx| fx.match_id == "ec.color.tint"));
        let applied = pixels(&mut h.state_mut().session);
        assert_ne!(original, applied, "the registered Tint must change rendered pixels");
        h.state_mut().session.execute("edit.undo", json!({})).unwrap();
        assert!(h.state().session.project.same_content(&before));
        h.state_mut().session.execute("edit.redo", json!({})).unwrap();
        assert_eq!(applied, pixels(&mut h.state_mut().session));
        h.state_mut().session.execute("edit.undo", json!({})).unwrap();
        h.run_steps(2);
    }
}

#[test]
fn arrows_select_current_results_escape_releases_focus_and_empty_selection_is_safe() {
    let mut h = harness(session());
    invoke(&mut h, "quickEffects.open", json!({"query":"blur"})).unwrap();
    h.run_steps(3);
    press(&mut h, Key::ArrowDown, Modifiers::NONE);
    let rows = invoke(&mut h, "quickEffects.results", json!({})).unwrap();
    assert_eq!(rows["results"][1]["selected"], true);
    press(&mut h, Key::ArrowUp, Modifiers::NONE);
    let rows = invoke(&mut h, "quickEffects.results", json!({})).unwrap();
    assert_eq!(rows["results"][0]["selected"], true);
    let before = h.state().session.project.clone();
    invoke(&mut h, "quickEffects.results", json!({"query":"no such imaginary effect"})).unwrap();
    h.run_steps(2);
    press(&mut h, Key::Enter, Modifiers::NONE);
    assert!(h.state().session.project.same_content(&before));
    assert_eq!(h.state().dialog, Some(Dialog::QuickEffects));
    press(&mut h, Key::Escape, Modifiers::NONE);
    assert_eq!(h.state().dialog, None);
    assert!(!h.ctx.memory(|m| m.has_focus(egui::Id::new("quick-effects-search"))));
    h.state_mut().session.execute("layer.select", json!({"layers":[]})).unwrap();
    invoke(&mut h, "quickEffects.open", json!({"query":"tint"})).unwrap();
    h.run_steps(2);
    press(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(h.state().dialog, Some(Dialog::QuickEffects));
    assert!(h.state().ui.status.contains("Select a layer"));
    assert!(h.state().session.project.same_content(&before));
}

#[test]
fn favorites_aliases_frequency_settings_and_custom_shortcut_survive_reload() {
    let store = Arc::new(MemoryConfig::default());
    let mut s = session();
    s.config = Some(store.clone());
    let mut h = harness(s);
    invoke(&mut h, "quickEffects.open", json!({"query":"tint"})).unwrap();
    h.run_steps(3);
    click(&mut h, "quickEffects.favorite.0");
    invoke(&mut h, "quickEffects.alias", json!({"key":"effect:ec.color.tint","alias":"minha cor"})).unwrap();
    let rows = invoke(&mut h, "quickEffects.results", json!({"query":"minha cor"})).unwrap();
    assert_eq!(rows["results"][0]["entry"]["id"], "ec.color.tint");
    invoke(&mut h, "quickEffects.apply", json!({})).unwrap();
    h.state_mut()
        .session
        .execute("prefs.set", json!({"values":{"quickEffects.maxResults":7,"quickEffects.animations":false,"quickEffects.opacity":0.85}}))
        .unwrap();
    h.state_mut().session.execute("shortcuts.preset", json!({"op":"duplicate","name":"Original Console Keys"})).unwrap();
    h.state_mut().session.execute("shortcuts.set", json!({"command":"quickEffects.open","keys":"Ctrl+Alt+F8"})).unwrap();
    h.run_steps(3);
    let (mods, key) = menus::parse_shortcut("Ctrl+Alt+F8").unwrap();
    press(&mut h, key, mods);
    assert_eq!(h.state().dialog, Some(Dialog::QuickEffects));
    press(&mut h, Key::Escape, Modifiers::NONE);
    let mut restored = Session { config: Some(store.clone()), ..Default::default() };
    restored.load_settings();
    assert_eq!(restored.prefs.quick_effects.max_results, 7);
    assert!(!restored.prefs.quick_effects.animations);
    assert_eq!(restored.prefs.quick_effects.usage["effect:ec.color.tint"], 1);
    assert!(restored.prefs.quick_effects.favorites.contains(&"effect:ec.color.tint".into()));
    assert_eq!(restored.prefs.quick_effects.aliases["effect:ec.color.tint"], "minha cor");
    assert!(store.read(effectcraft_engine::prefs::PREFS_FILE).unwrap().contains("minha cor"));
    drop(h);
    let mut h = harness(restored);
    let (mods, key) = menus::parse_shortcut("Ctrl+Alt+F8").unwrap();
    press(&mut h, key, mods);
    assert_eq!(h.state().dialog, Some(Dialog::QuickEffects), "custom binding survives settings reload");

    press(&mut h, Key::Escape, Modifiers::NONE);
    invoke(&mut h, "quickEffects.open", json!({"query":"tint"})).unwrap();
    h.run_steps(4);
    click(&mut h, "quickEffects.favorite.0");
    assert!(
        !h.state().session.prefs.quick_effects.favorites.contains(&"effect:ec.color.tint".into()),
        "favorites: {:?}, dialog: {:?}, recent: {:?}",
        h.state().session.prefs.quick_effects.favorites,
        h.state().dialog,
        h.state().session.prefs.quick_effects.recent
    );
    press(&mut h, Key::Escape, Modifiers::NONE);
    h.state_mut().session.execute("prefs.set", json!({"key":"quickEffects.enabled","value":false})).unwrap();
    assert!(invoke(&mut h, "quickEffects.open", json!({})).is_err());
    assert_eq!(h.state().dialog, None);
}

#[test]
fn native_user_and_builtin_presets_use_existing_commands() {
    let root = std::env::temp_dir().join(format!("cortez-console-native-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("Original Native Tint.ecpreset");
    let mut s = session();
    s.execute("effect.apply", json!({"effect":"ec.color.tint"})).unwrap();
    s.execute("anim.savePreset", json!({"path":path})).unwrap();
    s.execute("edit.undo", json!({})).unwrap();
    s.prefs.quick_effects.preset_folder = root.to_string_lossy().into_owned();
    let mut h = harness(s);
    invoke(&mut h, "quickEffects.open", json!({"query":"Original Native Tint"})).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        h.run_steps(2);
        let rows = invoke(&mut h, "quickEffects.results", json!({})).unwrap();
        if !rows["results"].as_array().unwrap().is_empty() {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let original = pixels(&mut h.state_mut().session);
    press(&mut h, Key::Enter, Modifiers::NONE);
    assert_ne!(original, pixels(&mut h.state_mut().session));
    assert_eq!(h.state().dialog, None);
    h.state_mut().session.execute("edit.undo", json!({})).unwrap();
    assert_eq!(original, pixels(&mut h.state_mut().session));
    invoke(&mut h, "quickEffects.open", json!({})).unwrap();
    let rows = invoke(&mut h, "quickEffects.results", json!({"filter":"presets"})).unwrap();
    assert!(!rows["results"].as_array().unwrap().is_empty());

    invoke(&mut h, "quickEffects.results", json!({"query":"Fade In", "filter":"presets"})).unwrap();
    invoke(&mut h, "quickEffects.apply", json!({"key":"builtin:fadeIn"})).unwrap();
    assert!(!h.state().session.active_comp().unwrap().layers[0].props.prop("transform/opacity").unwrap().keys.is_empty());
    assert_ne!(original, pixels(&mut h.state_mut().session));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn capture_png_and_clipboard_commands_render_the_composition() {
    let mut h = harness(session());
    let path = std::env::temp_dir().join(format!("cortez-console-capture-{}.png", std::process::id()));
    let original = pixels(&mut h.state_mut().session);
    let before = h.state().session.project.clone();
    invoke(&mut h, "quickEffects.open", json!({})).unwrap();
    let result = invoke(&mut h, "quickEffects.capture", json!({"path":path})).unwrap();
    assert_eq!(result["width"], 32);
    assert_eq!(result["height"], 24);
    let png = image::load_from_memory(&std::fs::read(&path).unwrap()).unwrap().to_rgba8();
    assert_eq!(png.as_raw(), &original);
    assert!(h.state().session.project.same_content(&before));
    let result = invoke(&mut h, "quickEffects.copyFrame", json!({})).unwrap();
    assert_eq!(result["width"], 32);
    assert!(h.state().session.events.iter().any(|event| matches!(event,effectcraft_engine::Event::CopyImage{width:32,height:24,rgba} if rgba==&original)));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn console_respects_themes_dpi_settings_and_ime_composition() {
    let mut h = harness(session());
    for (theme, dpi) in [("dark", 1.0), ("light", 1.5), ("darker", 2.0)] {
        h.state_mut().session.execute("prefs.set", json!({"key":"appearance.theme","value":theme})).unwrap();
        h.ctx.set_pixels_per_point(dpi);
        invoke(&mut h, "quickEffects.open", json!({"query":"warp"})).unwrap();
        h.run_steps(4);
        for id in ["quickEffects.search", "quickEffects.result.0", "quickEffects.close", "quickEffects.capture"] {
            let rect = h.state().auto.find(id).unwrap().rect;
            let content = h.ctx.content_rect();
            assert!(rect[0] >= content.left() - 2.0 && rect[0] + rect[2] <= content.right() + 2.0, "{id} at {dpi}: {rect:?} / {content:?}");
        }
        press(&mut h, Key::Escape, Modifiers::NONE);
    }
    invoke(&mut h, "quickEffects.open", json!({})).unwrap();
    h.run_steps(2);
    h.input_mut().events.push(Event::Ime(egui::ImeEvent::Preedit { text: "nihao".into(), active_range_chars: None }));
    h.run_steps(2);
    press(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(h.state().dialog, Some(Dialog::QuickEffects));
    assert_eq!(h.state().session.active_comp().unwrap().layers[0].effects().unwrap().groups().count(), 0, "IME Enter must not apply a result");
}

#[test]
#[ignore = "explicit original generated FFX and separate converter required; no private fixture redistribution"]
fn ffx_console_uses_existing_importer_and_preserves_its_diagnostics() {
    let path = std::env::var("EFFECTCRAFT_TEST_FFX").unwrap();
    let mut s = session();
    s.ae_converter = effectcraft_host::session().ae_converter;
    assert!(s.ae_converter.is_some(), "edition converter must be configured through the real host");
    s.prefs.recent_presets.push(path.clone());
    let mut h = harness(s);
    invoke(&mut h, "quickEffects.open", json!({})).unwrap();
    h.run_steps(2);
    let key = effectcraft_engine::quick_effects::Entry::file(&path).unwrap().key;
    let result = invoke(&mut h, "quickEffects.apply", json!({"key":key})).unwrap();
    assert_eq!(result["applied"], true);
    assert!(result["propertiesApplied"].as_u64().unwrap() > 0);
    h.run_steps(3);
    assert_eq!(h.state().dialog, Some(Dialog::Info));
    assert!(h.state().auto.find("report.exportJson").is_some());
    assert!(h.state().session.active_comp().unwrap().layers[0].comment.contains("EffectCraft FFX preservation:"));
    assert!(h.state().session.prefs.recent_presets.contains(&path));
    let repeated = h.state_mut().session.execute("anim.applyRecentPreset", json!({"index":0})).unwrap();
    assert_eq!(repeated["applied"], true, "the existing Recent Presets menu must dispatch FFX to the converter");
}

#[test]
fn configured_capture_folder_uses_scale_and_distinct_names_and_rejects_oversized_capture_before_rendering() {
    let root = std::env::temp_dir().join(format!("cortez-console-folder-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut source = session();
    source.prefs.quick_effects.screenshot_folder = root.to_string_lossy().into_owned();
    source.prefs.quick_effects.screenshot_scale = 0.5;
    let mut app = EffectcraftApp::new(source);
    let ctx = egui::Context::default();
    let first = menus::invoke(&mut app, &ctx, "quickEffects.capture", json!({})).unwrap();
    let second = menus::invoke(&mut app, &ctx, "quickEffects.capture", json!({})).unwrap();
    assert_ne!(first["path"], second["path"]);
    for frame in [first, second] {
        let path = std::path::Path::new(frame["path"].as_str().unwrap());
        assert_eq!(path.parent(), Some(root.as_path()));
        let image = image::open(path).unwrap();
        assert_eq!((image.width(), image.height()), (16, 12));
    }
    let mut large = Session::default();
    large.execute("comp.new", json!({"width":8192,"height":8192})).unwrap();
    let mut app = EffectcraftApp::new(large);
    let result = menus::invoke(&mut app, &ctx, "quickEffects.capture", json!({"path":root.join("must-not-exist.png")}));
    assert!(result.unwrap_err().contains("16 megapixels"));
    assert!(!root.join("must-not-exist.png").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_preset_reports_unavailable_effects_and_does_not_claim_empty_presets_were_applied() {
    let root = std::env::temp_dir().join(format!("cortez-console-partial-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("Original.ecpreset");
    let mut source = session();
    source.execute("effect.apply", json!({"effect":"ec.color.tint"})).unwrap();
    source.execute("anim.savePreset", json!({"path":path})).unwrap();
    let original: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut missing: effectcraft_engine::project::PropGroup = serde_json::from_value(original["effects"][0].clone()).unwrap();
    missing.match_id = "org.cortezeffects.test.unavailable".into();
    source.execute("edit.undo", json!({})).unwrap();
    let before = source.project.clone();
    let original_pixels = pixels(&mut source);
    let mut h = harness(source);
    for (name, effects, works) in
        [("Empty", json!([]), false), ("Unavailable", json!([missing]), false), ("Partial", json!([original["effects"][0], missing]), true)]
    {
        let path = root.join(format!("{name}.ecpreset"));
        let mut document = original.clone();
        document["effects"] = effects;
        document["properties"] = json!([]);
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        h.state_mut().session.prefs.recent_presets.push(path.to_string_lossy().into_owned());
        invoke(&mut h, "quickEffects.open", json!({})).unwrap();
        let key = effectcraft_engine::quick_effects::Entry::file(&path.to_string_lossy()).unwrap().key;
        let result = invoke(&mut h, "quickEffects.apply", json!({"key":key}));
        assert_eq!(result.is_ok(), works, "{name}: {result:?}");
        h.run_steps(3);
        assert_eq!(h.state().dialog, Some(Dialog::Info));
        if works {
            assert_ne!(original_pixels, pixels(&mut h.state_mut().session));
        }
        if name != "Empty" {
            assert!(
                h.state().session.active_comp().unwrap().layers[0]
                    .effects()
                    .unwrap()
                    .groups()
                    .any(|group| group.match_id == "org.cortezeffects.test.unavailable")
            );
            h.state_mut().session.execute("edit.undo", json!({})).unwrap();
        }
        assert!(h.state().session.project.same_content(&before));
        press(&mut h, Key::Escape, Modifiers::NONE);
    }
    std::fs::remove_dir_all(root).unwrap();
}
