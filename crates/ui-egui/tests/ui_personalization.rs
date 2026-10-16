use std::sync::Arc;

use effectcraft_engine::{Session, config::MemoryConfig, prefs::Prefs};
use effectcraft_ui_egui::{
    EffectcraftApp, menus,
    theme::{ThemeKind, Tokens},
};
use egui::{Event, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;

#[test]
fn personalization_round_trips_without_changing_legacy_settings_or_project() {
    let store = Arc::new(MemoryConfig::default());
    let mut session = Session { config: Some(store.clone()), ..Default::default() };
    session.execute("comp.new", json!({"name":"Preserved","width":32,"height":32,"duration":2})).unwrap();
    session.execute("layer.newSolid", json!({"color":"#406080"})).unwrap();
    let before = serde_json::to_value(&session.project).unwrap();
    let comp = session.active_comp_id().unwrap();
    let frame_before = session.render_rgba8(comp, effectcraft_engine::time::Tick::ZERO, 32).unwrap();
    let mut app = EffectcraftApp::new(session);
    for (key, value) in [
        ("theme", json!("graphite")),
        ("accentColor", json!("#60d8ed")),
        ("wallpaperPath", json!("C:\\Users\\Editor\\Pictures\\background.png")),
        ("wallpaperFit", json!("contain")),
        ("glassBlur", json!(17.0)),
        ("panelOpacity", json!(0.8)),
        ("glow", json!(0.4)),
        ("reflections", json!(0.6)),
        ("cornerRadius", json!(14.0)),
        ("shadows", json!(0.2)),
        ("surfaceSaturation", json!(0.5)),
        ("wallpaperOpacity", json!(0.7)),
        ("wallpaperBlur", json!(4.0)),
        ("wallpaperDim", json!(0.2)),
        ("fontFamily", json!("mono")),
        ("textScale", json!(1.15)),
        ("uiScale", json!(125)),
        ("reduceMotion", json!(true)),
        ("lowVisualPower", json!(true)),
        ("adaptiveEffects", json!(false)),
    ] {
        app.set_pref(&format!("appearance.{key}"), value).unwrap();
    }
    let mut reopened = Session { config: Some(store), ..Default::default() };
    reopened.load_settings();
    assert_eq!(reopened.prefs.appearance, app.session.prefs.appearance);
    assert_eq!(before, serde_json::to_value(&app.session.project).unwrap());
    for theme in ["dark", "light", "darker", "graphite", "classic"] {
        app.set_pref("appearance.theme", json!(theme)).unwrap();
        let frame_after = app.session.render_rgba8(comp, effectcraft_engine::time::Tick::ZERO, 32).unwrap();
        assert_eq!(frame_after, frame_before, "personalization must not alter rendered pixels: {theme}");
    }
    assert_eq!(reopened.prefs.appearance.ui_scale, 125);
    let tokens = Tokens::from_prefs(&reopened.prefs, ThemeKind::Graphite);
    assert_eq!(tokens.motion, 0.0);
    assert!(!tokens.glass);
    assert_eq!((tokens.glow, tokens.reflections, tokens.shadows), (0.0, 0.0, 0.0));
    app.session.prefs.reset(Some("appearance")).unwrap();
    assert_eq!(app.session.prefs.appearance, Prefs::default().appearance);
}

#[test]
fn legacy_preferences_keep_paths_unknown_fields_and_theme_family() {
    let p =
        Prefs::from_json(r##"{"appearance":{"theme":"darker","futureOption":42},"recentProjects":["C:\\Old\\project.ecproj"],"general":{"undoLevels":63}}"##);
    assert_eq!(p.appearance.dark_theme, "darker");
    assert_eq!(p.appearance.extra.get("futureOption"), Some(&json!(42)));
    assert_eq!(p.general.undo_levels, 63);
    assert_eq!(p.recent_projects, vec!["C:\\Old\\project.ecproj"]);
    for name in ["dark", "light", "darker", "graphite", "classic"] {
        let kind = ThemeKind::from_name(name).unwrap();
        let tokens = Tokens::from_prefs(&p, kind);
        assert_eq!(tokens.kind, kind);
        assert_eq!(tokens.pasteboard.a(), 255, "preview pasteboard must remain opaque");
        assert_eq!(tokens.tl_bg.a(), 255, "time graph remains legible");
    }
    let p = Prefs::from_json(r##"{"appearance":{"accentColor":"bad","panelOpacity":-8,"glassBlur":999,"textScale":100,"wallpaperFit":"bad"}}"##);
    assert_eq!(p.appearance.accent_color, "#9b87ff");
    assert_eq!(p.appearance.panel_opacity, 0.65);
    assert_eq!(p.appearance.glass_blur, 24.0);
    assert_eq!(p.appearance.text_scale, 1.5);
    assert_eq!(p.appearance.wallpaper_fit, "cover");
}

#[test]
fn appearance_shortcut_and_swatch_are_real_controls_and_cancel_persists_baseline() {
    let store = Arc::new(MemoryConfig::default());
    let session = Session { config: Some(store.clone()), ..Default::default() };
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_eframe(|_| EffectcraftApp::new(session));
    h.run_steps(4);
    assert!(h.state().auto.find("header.personalization").is_some());
    let ctx = h.ctx.clone();
    menus::invoke(h.state_mut(), &ctx, "app.settings", json!({"page":"appearance"})).unwrap();
    h.run_steps(3);
    for id in ["settings.appearance.accent.Cyan", "settings.cancel"] {
        let [x, y, w, z] = h.state().auto.find(id).unwrap().rect;
        let pos = pos2(x + w / 2.0, y + z / 2.0);
        h.event(Event::PointerMoved(pos));
        h.step();
        for pressed in [true, false] {
            h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() });
            h.step();
        }
        h.run_steps(2);
        let mut saved = Session { config: Some(store.clone()), ..Default::default() };
        saved.load_settings();
        assert_eq!(saved.prefs.appearance.accent_color, if id.ends_with("Cyan") { "#60d8ed" } else { "#9b87ff" });
    }
}
