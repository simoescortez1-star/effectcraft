//! Native Quick Effects Console. Registry, transactions, presets and frame capture use existing commands.
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::Receiver;

use effectcraft_engine::quick_effects::{Entry, Kind};
use egui::{Align2, Color32, Key, Rect, RichText, Sense, Stroke, pos2, vec2};
use serde_json::{Value, json};

use crate::i18n::tr;
use crate::theme::Tokens;
use crate::{Dialog, EffectcraftApp};

#[derive(Default)]
pub struct State {
    query: String,
    selected: usize,
    filter: String,
    focus: bool,
    ime: bool,
    opened: f64,
    catalog_key: u64,
    generation: u64,
    cached_key: Option<(String, String, u64, u64)>,
    cached_results: Vec<usize>,
    entries: Vec<Entry>,
    names: Vec<String>,
    files: Vec<Entry>,
    roots: Vec<String>,
    pending: Option<Receiver<Scan>>,
    scanned_at: f64,
    warning: String,
    error: String,
    alias: String,
    alias_key: String,
    edit_alias: bool,
}

struct Scan {
    files: Vec<Entry>,
    warnings: Vec<String>,
}

/// Metadata-only bounded traversal. Never opens a preset or follows directory symlinks.
#[cfg(any(test, not(target_arch = "wasm32")))]
fn scan(roots: &[String]) -> Scan {
    let mut out = Scan { files: vec![], warnings: vec![] };
    let mut pending: Vec<_> = roots.iter().map(|root| (std::path::PathBuf::from(root), 0)).collect();
    let mut seen = BTreeSet::new();
    let mut visited = 0_usize;
    while let Some((dir, depth)) = pending.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let read = match std::fs::read_dir(&dir) {
            Ok(read) => read,
            Err(e) => {
                if out.warnings.len() < 16 && !(e.kind() == std::io::ErrorKind::NotFound && super::effects_presets::presets_dir().as_ref() == Some(&dir)) {
                    out.warnings.push(format!("{}: {e}", dir.display()));
                }
                continue;
            }
        };
        for entry in read {
            visited = visited.saturating_add(1);
            if visited > 10_000 || out.files.len() >= 2_000 {
                out.warnings.push("Preset index limit reached (2,000 presets / 10,000 directory entries).".into());
                return out;
            }
            let Ok(entry) = entry else { continue };
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() && depth < 4 {
                pending.push((entry.path(), depth + 1));
            }
            if kind.is_file()
                && let Some(file) = Entry::file(&entry.path().to_string_lossy())
            {
                out.files.push(file);
            }
        }
    }
    out.files.sort_by_key(|entry| entry.name.to_lowercase());
    out
}

fn sync(app: &EffectcraftApp, ctx: &egui::Context, state: &mut State, force: bool) {
    let mut dirty = force;
    if let Some(receiver) = &state.pending {
        match receiver.try_recv() {
            Ok(result) => {
                state.files = result.files;
                state.warning = result.warnings.into_iter().take(3).collect::<Vec<_>>().join("\n");
                state.pending = None;
                dirty = true;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.pending = None;
                state.warning = "Preset indexing worker stopped; use Refresh to retry.".into();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    let mut roots = vec![];
    if let Some(dir) = super::effects_presets::presets_dir() {
        roots.push(dir.to_string_lossy().into_owned());
    }
    let extra = &app.session.prefs.quick_effects.preset_folder;
    if !extra.is_empty() {
        roots.push(extra.clone());
    }
    roots.sort();
    roots.dedup();
    let now = ctx.input(|input| input.time);
    if state.pending.is_none() && (force || state.roots != roots || now - state.scanned_at >= 5.0) {
        state.roots = roots.clone();
        state.scanned_at = now;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            let repaint = ctx.clone();
            match std::thread::Builder::new().name("quick-effects-presets".into()).spawn(move || {
                let result = scan(&roots);
                let _ = tx.send(result);
                repaint.request_repaint();
            }) {
                Ok(_) => state.pending = Some(rx),
                Err(e) => state.warning = format!("Cannot index presets: {e}"),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            state.warning = "Local preset folder indexing requires the desktop application.".into();
        }
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for spec in effectcraft_engine::effects::all() {
        spec.id.hash(&mut hash);
        spec.name.hash(&mut hash);
    }
    app.session.prefs.general.language.hash(&mut hash);
    app.session.prefs.recent_presets.hash(&mut hash);
    let key = hash.finish();
    if dirty || state.catalog_key != key {
        state.entries = effectcraft_engine::quick_effects::catalog();
        state.entries.extend(state.files.iter().cloned());
        state.entries.extend(app.session.prefs.recent_presets.iter().filter_map(|path| Entry::file(path)));
        let mut seen = BTreeSet::new();
        state.entries.retain(|entry| seen.insert(entry.key.clone()));
        state.names = state
            .entries
            .iter()
            .map(|entry| if entry.kind == Kind::Effect { crate::i18n::effects::name(&entry.id, &entry.name) } else { entry.name.clone() })
            .collect();
        state.catalog_key = key;
        state.generation = state.generation.saturating_add(1);
    }
    if state.pending.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}

fn results(state: &mut State, app: &EffectcraftApp) -> Vec<usize> {
    let key = (state.query.clone(), state.filter.clone(), app.session.prefs_revision, state.generation);
    if state.cached_key.as_ref() != Some(&key) {
        state.cached_results =
            effectcraft_engine::quick_effects::search(&state.entries, &state.names, &state.query, &app.session.prefs.quick_effects, &state.filter);
        state.cached_key = Some(key);
    }
    state.cached_results.clone()
}

fn save(app: &mut EffectcraftApp) {
    app.session.prefs_changed();
    app.session.save_prefs();
}

pub fn favorite(app: &mut EffectcraftApp, key: &str) -> bool {
    if let Some(id) = key.strip_prefix("effect:")
        && app.ui.effects_favorites.contains(id)
        && !app.session.prefs.quick_effects.favorites.iter().any(|entry| entry == key)
    {
        app.session.prefs.quick_effects.favorites.push(key.into());
    }

    if !app.session.prefs.quick_effects.favorites.iter().any(|entry| entry == key) && app.session.prefs.quick_effects.favorites.len() >= 256 {
        app.ui.status = "Favorite limit reached (256); remove another favorite first.".into();
        return false;
    }
    let on = app.session.prefs.quick_effects.favorite(key);
    if let Some(id) = key.strip_prefix("effect:") {
        if on {
            app.ui.effects_favorites.insert(id.into());
        } else {
            app.ui.effects_favorites.remove(id);
        }
    }
    save(app);
    on
}

pub fn is_favorite(app: &EffectcraftApp, id: &str) -> bool {
    app.session.prefs.quick_effects.favorites.iter().any(|key| key == &format!("effect:{id}")) || app.ui.effects_favorites.contains(id)
}

pub(crate) fn close(app: &mut EffectcraftApp, ctx: &egui::Context) {
    if app.dialog == Some(Dialog::QuickEffects) {
        app.dialog = None;
    }
    ctx.memory_mut(|memory| memory.surrender_focus(egui::Id::new("quick-effects-search")));
}

fn apply(app: &mut EffectcraftApp, ctx: &egui::Context, key: &str) -> Result<Value, String> {
    if app.session.state.selected_layers.is_empty() {
        return Err(tr("Select a layer to apply to").into());
    }
    let entry = app.quick_console.entries.iter().find(|entry| entry.key == key).cloned().ok_or("The selected effect or preset is no longer in the catalog")?;
    let prior: BTreeSet<_> = if entry.kind == Kind::NativePreset {
        app.session
            .active_comp()
            .into_iter()
            .flat_map(|comp| comp.layers.iter())
            .flat_map(|layer| layer.effects().into_iter().flat_map(|effects| effects.groups()))
            .map(|effect| effect.uid)
            .collect()
    } else {
        BTreeSet::new()
    };
    let (command, params) = entry.command();
    let mut result = crate::menus::invoke(app, ctx, command, params)?;
    if entry.kind == Kind::NativePreset {
        let missing: Vec<_> = app
            .session
            .active_comp()
            .into_iter()
            .flat_map(|comp| comp.layers.iter())
            .flat_map(|layer| layer.effects().into_iter().flat_map(|effects| effects.groups()))
            .filter(|effect| !prior.contains(&effect.uid) && effectcraft_engine::effects::find(&effect.match_id).is_none())
            .map(|effect| effect.match_id.clone())
            .collect();
        let count = result.get("applied").and_then(Value::as_u64).unwrap_or(0);
        let skipped = result.get("skipped").and_then(Value::as_array).map_or(0, Vec::len);
        if !missing.is_empty() || skipped > 0 || count == 0 {
            result["unavailableEffects"] = json!(missing);
            app.session.events.push(effectcraft_engine::Event::Frontend { command:"app.showReport".into(), params:json!({"title":"Native Preset Report", "summary":format!("{}: {} mapped item(s); {} skipped properties; {} unavailable effect(s). Unavailable effects remain preserved in the project.",entry.name,count.saturating_sub(missing.len() as u64),skipped,missing.len()),"text":serde_json::to_string_pretty(&result).map_err(|error|error.to_string())?}) });
        }
        if count == 0 || count <= missing.len() as u64 {
            return Err(tr("No supported property or effect was applied. See the preset report.").into());
        }
    }

    if result.get("applied") == Some(&Value::Bool(false)) {
        return Err(tr("Preset data was preserved; no supported property or effect was applied. See the import report.").into());
    }
    app.session.prefs.quick_effects.record(key);
    save(app);
    if entry.kind == Kind::Effect {
        super::effects_presets::push_recent(&mut app.ui.effects_recent, &entry.id);
    }
    app.show_panel(crate::dock::PanelKind::EffectControls);
    app.ui.focused = crate::dock::PanelKind::EffectControls;
    close(app, ctx);
    Ok(result)
}

/// All actions are accessible to shortcuts and the control channel.
pub fn command(app: &mut EffectcraftApp, ctx: &egui::Context, id: &str, p: Value) -> Result<Value, String> {
    match id {
        "quickEffects.close" => {
            close(app, ctx);
            Ok(json!({"open":false}))
        }
        "quickEffects.open" => {
            if !app.session.prefs.quick_effects.enabled {
                return Err(tr("Quick Effects Console is disabled in Settings.").into());
            }
            if app.dialog.is_some() && app.dialog != Some(Dialog::QuickEffects) {
                return Err(tr("Close the current dialog first.").into());
            }
            let mut changed = false;
            for effect in &app.ui.effects_favorites {
                let key = format!("effect:{effect}");
                if !app.session.prefs.quick_effects.favorites.contains(&key) {
                    app.session.prefs.quick_effects.favorites.push(key);
                    changed = true;
                }
            }
            if changed {
                app.session.prefs.quick_effects.normalize();
                save(app);
            }
            let mut state = std::mem::take(&mut app.quick_console);
            state.query = p.get("query").and_then(Value::as_str).unwrap_or("").chars().take(128).collect();
            state.selected = 0;
            state.filter = "all".into();
            state.focus = true;
            state.ime = false;
            state.error.clear();
            state.edit_alias = false;
            state.alias_key.clear();
            state.opened = ctx.input(|i| i.time);
            sync(app, ctx, &mut state, true);
            app.quick_console = state;
            app.dialog = Some(Dialog::QuickEffects);
            ctx.request_repaint();
            Ok(json!({"open":true}))
        }
        "quickEffects.results" | "quickEffects.refresh" => {
            let mut state = std::mem::take(&mut app.quick_console);
            sync(app, ctx, &mut state, id == "quickEffects.refresh");
            if let Some(query) = p.get("query").and_then(Value::as_str) {
                state.query = query.chars().take(128).collect();
                state.selected = 0;
            }
            if let Some(filter) = p.get("filter").and_then(Value::as_str) {
                state.filter = filter.into();
                state.selected = 0;
            }
            let entries: Vec<_> = results(&mut state, app)
                .into_iter()
                .enumerate()
                .filter_map(|(row, index)| {
                    state.entries.get(index).map(
                        |entry| json!({"entry":entry,"selected":row==state.selected,"favorite":app.session.prefs.quick_effects.favorites.contains(&entry.key)}),
                    )
                })
                .collect();
            let result = json!({"open":app.dialog==Some(Dialog::QuickEffects),"query":state.query,"indexing":state.pending.is_some(),"warning":state.warning,"error":state.error,"results":entries});
            app.quick_console = state;
            Ok(result)
        }
        "quickEffects.apply" => {
            let mut state = std::mem::take(&mut app.quick_console);
            let key = p
                .get("key")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| results(&mut state, app).get(state.selected).and_then(|index| state.entries.get(*index)).map(|entry| entry.key.clone()));
            app.quick_console = state;
            let key = key.ok_or("No matching effect or preset")?;
            apply(app, ctx, &key)
        }
        "quickEffects.favorite" => {
            let key = p.get("key").and_then(Value::as_str).ok_or("missing favorite key")?;
            if !app.quick_console.entries.iter().any(|entry| entry.key == key) {
                return Err("Unknown favorite entry".into());
            }
            if !app.session.prefs.quick_effects.favorites.iter().any(|entry| entry == key) && app.session.prefs.quick_effects.favorites.len() >= 256 {
                return Err("Favorite limit reached (256); remove another favorite first.".into());
            }
            Ok(json!({"favorite":favorite(app,key)}))
        }
        "quickEffects.alias" => {
            let key = p.get("key").and_then(Value::as_str).ok_or("missing alias key")?;
            if !app.quick_console.entries.iter().any(|entry| entry.key == key) {
                return Err("Unknown alias entry".into());
            }
            let alias = p.get("alias").and_then(Value::as_str).ok_or("missing alias")?.trim();
            if alias.len() > 512 {
                return Err("Alias exceeds 512 bytes".into());
            }
            if !alias.is_empty() && !app.session.prefs.quick_effects.aliases.contains_key(key) && app.session.prefs.quick_effects.aliases.len() >= 256 {
                return Err("Alias limit reached (256); remove another alias first.".into());
            }
            if alias.is_empty() {
                app.session.prefs.quick_effects.aliases.remove(key);
            } else {
                app.session.prefs.quick_effects.aliases.insert(key.into(), alias.into());
            }
            app.session.prefs.quick_effects.normalize();
            save(app);
            Ok(json!({"saved":true}))
        }
        "quickEffects.shortcut" => {
            close(app, ctx);
            super::shortcut_editor::open(app);
            app.dialog_state.shortcuts.query = if p.get("capture") == Some(&Value::Bool(true)) {
                "Console: Capture".into()
            } else {
                p.get("effect").and_then(Value::as_str).unwrap_or("Quick Effects Console").into()
            };
            Ok(json!({"dialog":"shortcuts"}))
        }
        "quickEffects.importFfx" => {
            if app.session.state.selected_layers.is_empty() {
                return Err(tr("Select a layer to apply to").into());
            }
            close(app, ctx);
            crate::menus::invoke(app, ctx, "anim.applyAfterEffectsPreset", json!({}))
        }
        "quickEffects.capture" | "quickEffects.copyFrame" => {
            let comp = app.session.active_comp().ok_or("No composition is open")?;
            let scale = app.session.prefs.quick_effects.screenshot_scale;
            let pixels = ((f64::from(comp.width) * scale).ceil() as u64).checked_mul((f64::from(comp.height) * scale).ceil() as u64);
            if pixels.is_none_or(|pixels| pixels > 16 * 1024 * 1024) {
                return Err(tr("Capture exceeds 16 megapixels. Reduce Screenshot Scale in Settings.").into());
            }

            let mut params = json!({"scale":app.session.prefs.quick_effects.screenshot_scale});
            if id == "quickEffects.capture" {
                if let Some(path) = p.get("path").and_then(Value::as_str) {
                    params["path"] = json!(path);
                } else if !app.session.prefs.quick_effects.screenshot_folder.is_empty() {
                    #[cfg(target_arch = "wasm32")]
                    return Err("Configured screenshot folders require the desktop application".into());
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let cid = app.session.active_comp_id().ok_or("No composition is open")?;
                        let comp = app.session.project.item(cid).ok_or("No composition is open")?;
                        let name: String = comp.name.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).take(64).collect();
                        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
                        params["path"] = json!(
                            std::path::Path::new(&app.session.prefs.quick_effects.screenshot_folder)
                                .join(format!("CortezEffects_{name}_{stamp}.png"))
                                .to_string_lossy()
                        );
                    }
                }
            }
            let command = if id == "quickEffects.capture" { "comp.saveFrameAs" } else { "comp.copyFrame" };
            let result = crate::menus::invoke(app, ctx, command, params)?;
            close(app, ctx);
            Ok(result)
        }
        _ => Err(format!("Unknown console command {id}")),
    }
}

pub fn show(app: &mut EffectcraftApp, ctx: &egui::Context) {
    let mut state = std::mem::take(&mut app.quick_console);
    sync(app, ctx, &mut state, false);
    let prefs = app.session.prefs.quick_effects.clone();
    let t = app.tokens;
    let width = (prefs.width as f32 - 36.0).min((ctx.content_rect().width() - 68.0).max(180.0));
    let mut action: Option<(&str, Value)> = None;
    let mut chosen = None;
    let mut close_now = false;
    let old_query = state.query.clone();
    let old_filter = state.filter.clone();
    let mut commit = false;
    ctx.input(|input| {
        for event in &input.events {
            if let egui::Event::Ime(ime) = event {
                match ime {
                    egui::ImeEvent::Preedit { text, .. } => state.ime = !text.is_empty(),
                    egui::ImeEvent::Commit(_) => {
                        state.ime = false;
                        commit = true;
                    }
                    _ => state.ime = false,
                }
            }
        }
    });
    let owns_search = state.focus || ctx.memory(|memory| memory.has_focus(egui::Id::new("quick-effects-search")));
    let (down, up, enter) = if !owns_search || state.ime || commit || state.edit_alias {
        (false, false, false)
    } else {
        ctx.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
                input.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
                input.consume_key(egui::Modifiers::NONE, Key::Enter),
            )
        })
    };
    let progress = if prefs.animations && !app.session.prefs.appearance.reduce_motion {
        ((ctx.input(|i| i.time) - state.opened) / 0.12).clamp(0.0, 1.0) as f32
    } else {
        1.0
    };
    if progress < 1.0 {
        ctx.request_repaint();
    }
    let frame = egui::Frame::new()
        .fill(t.panel_bg.gamma_multiply(if t.glass { prefs.opacity as f32 } else { 1.0 }))
        .inner_margin(18.0)
        .corner_radius(if t.glass { t.radius.max(10.0) } else { t.radius } as u8)
        .stroke(Stroke::new(1.0, if t.glass { t.accent.gamma_multiply(0.45) } else { t.text_dim.gamma_multiply(0.25) }))
        .shadow(egui::Shadow { offset: [0, 12], blur: 40, spread: 2, color: Color32::from_black_alpha((110.0 * t.shadows) as u8) });
    egui::Modal::new(egui::Id::new("quick-effects-console")).frame(frame).backdrop_color(Color32::from_black_alpha(55)).show(ctx, |ui| {
        ui.set_width(width);
        let glass_shape = ui.painter().add(egui::Shape::Noop);
        ui.set_opacity(0.65 + 0.35 * progress);
        app.auto.add("quickEffects.console", ui.max_rect(), "Quick Effects Console");
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Quick Effects Console")).font(Tokens::semibold(16.0)).color(t.tab_text_active));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = ui.small_button("×");
                app.auto.add("quickEffects.close", r.rect, "Close");
                if r.clicked() {
                    close_now = true;
                }
                let r = ui.small_button(tr("Settings"));
                app.auto.add("quickEffects.settings", r.rect, "Settings");
                if r.clicked() {
                    action = Some(("prefs.open", json!({"page":"quickEffects"})));
                }
            });
        });
        ui.add_space(7.0);
        let response = ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .id(egui::Id::new("quick-effects-search"))
                .hint_text(tr("Search effects, presets or aliases…"))
                .font(Tokens::ui(16.0))
                .desired_width(f32::INFINITY),
        );
        app.auto.add("quickEffects.search", response.rect, "Search effects and presets");
        if state.focus {
            response.request_focus();
            state.focus = false;
        }
        state.query = state.query.chars().take(128).collect();
        ui.horizontal_wrapped(|ui| {
            for (key, label) in [("all", "All"), ("effects", "Effects"), ("presets", "Presets"), ("favorites", "Favorites"), ("recent", "Recent")] {
                if key == "recent" && !prefs.show_recent {
                    continue;
                }
                let r = ui.selectable_label(state.filter == key, tr(label));
                app.auto.add(&format!("quickEffects.filter.{key}"), r.rect, label);
                if r.clicked() {
                    state.filter = key.into();
                }
            }
        });
        if state.query != old_query || state.filter != old_filter {
            state.selected = 0;
        }
        let rows = results(&mut state, app);
        state.selected = state.selected.min(rows.len().saturating_sub(1));
        if down {
            state.selected = (state.selected + 1).min(rows.len().saturating_sub(1));
        }
        if up {
            state.selected = state.selected.saturating_sub(1);
        }
        let navigation = down || up;
        egui::ScrollArea::vertical().id_salt("quick-effects-results").max_height((ctx.content_rect().height() - 290.0).clamp(60.0, 650.0)).show(ui, |ui| {
            for (row, index) in rows.iter().enumerate() {
                let Some(entry) = state.entries.get(*index) else { continue };
                let selected = row == state.selected;
                let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
                if selected || response.hovered() {
                    ui.painter().rect_filled(rect, 7.0, if selected { t.row_selected } else { t.hover });
                }
                let icon = Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(18.0, 18.0));
                crate::icons::paint(ui.painter(), icon, if entry.kind == Kind::Effect { crate::icons::Icon::Fx } else { crate::icons::Icon::Folder }, t.accent);
                let name = state.names.get(*index).unwrap_or(&entry.name);
                let text_painter =
                    ui.painter().with_clip_rect(Rect::from_min_max(pos2(rect.left() + 36.0, rect.top()), pos2(rect.right() - 40.0, rect.bottom())));
                text_painter.text(pos2(rect.left() + 38.0, rect.top() + 13.0), Align2::LEFT_CENTER, name, Tokens::semibold(13.0), t.text);
                let status = if entry.kind == Kind::FfxPreset { " · FFX" } else { "" };
                text_painter.text(
                    pos2(rect.left() + 38.0, rect.top() + 32.0),
                    Align2::LEFT_CENTER,
                    format!("{}{}", entry.category, status),
                    Tokens::ui(10.5),
                    t.text_dim,
                );
                let favorite = prefs.favorites.contains(&entry.key);
                let star_rect = Rect::from_center_size(pos2(rect.right() - 19.0, rect.center().y), vec2(30.0, 30.0));
                let star = ui.interact(star_rect, egui::Id::new(("quick-effects-favorite", &entry.key)), Sense::click());
                super::effects_presets::filled_star(
                    ui.painter(),
                    star_rect.center(),
                    7.0,
                    favorite.then_some(t.accent),
                    if favorite { t.accent } else { t.text_dim },
                );
                let visible = rect.intersect(ui.clip_rect());
                if visible.is_positive() {
                    app.auto.add(&format!("quickEffects.result.{row}"), visible, &entry.name);
                    let star_visible = star_rect.intersect(ui.clip_rect());
                    if star_visible.is_positive() {
                        app.auto.add(&format!("quickEffects.favorite.{row}"), star_visible, if favorite { "Remove favorite" } else { "Add favorite" });
                    }
                }
                if star.clicked() {
                    action = Some(("quickEffects.favorite", json!({"key":entry.key})));
                } else if response.clicked() {
                    state.selected = row;
                    chosen = Some(entry.key.clone());
                }
                if selected && navigation {
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                }
            }
            if rows.is_empty() {
                ui.add_space(18.0);
                ui.label(tr("No matching effects or presets"));
                ui.add_space(18.0);
            }
        });
        if enter && let Some(entry) = rows.get(state.selected).and_then(|index| state.entries.get(*index)) {
            chosen = Some(entry.key.clone());
        }
        ui.separator();
        if let Some(entry) = rows.get(state.selected).and_then(|index| state.entries.get(*index)) {
            if state.alias_key != entry.key {
                state.alias_key = entry.key.clone();
                state.alias = prefs.aliases.get(&entry.key).cloned().unwrap_or_default();
            }
            ui.horizontal(|ui| {
                let r = ui.small_button(tr("Alias…"));
                app.auto.add("quickEffects.editAlias", r.rect, "Edit search alias");
                if r.clicked() {
                    state.edit_alias = !state.edit_alias;
                }
                let r = ui.add_enabled(entry.kind == Kind::Effect, egui::Button::new(tr("Shortcut…")).small());
                app.auto.add("quickEffects.effectShortcut", r.rect, "Configure selected effect shortcut");
                if r.clicked() {
                    action = Some(("quickEffects.shortcut", json!({"effect":entry.name})));
                }
                ui.add(egui::Label::new(RichText::new(&entry.id).font(Tokens::ui(10.0)).color(t.text_dim)).truncate()).on_hover_text(&entry.id);
            });
            if state.edit_alias {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut state.alias).hint_text(tr("Custom search words")));
                    app.auto.add("quickEffects.alias", r.rect, "Custom search words");
                    let r = ui.small_button(tr("Save"));
                    app.auto.add("quickEffects.saveAlias", r.rect, "Save alias");
                    if r.clicked() {
                        action = Some(("quickEffects.alias", json!({"key":state.alias_key,"alias":state.alias})));
                        state.edit_alias = false;
                        state.focus = true;
                    }
                });
            }
        }
        ui.horizontal_wrapped(|ui| {
            for (label, id) in [
                ("Capture PNG", "quickEffects.capture"),
                ("Copy Frame", "quickEffects.copyFrame"),
                ("Apply FFX…", "quickEffects.importFfx"),
                ("Refresh", "quickEffects.refresh"),
            ] {
                let r = ui.small_button(tr(label));
                app.auto.add(id, r.rect, label);
                if r.clicked() {
                    action = Some((id, json!({})));
                }
            }
            if state.pending.is_some() {
                ui.spinner();
            }
        });
        if !state.error.is_empty() {
            ui.colored_label(t.accent, &state.error);
        }
        if !state.warning.is_empty() {
            ui.collapsing(tr("Preset index details"), |ui| {
                ui.label(&state.warning);
            });
        }
        ui.label(RichText::new(tr("↑ ↓ Navigate   Enter Apply   Esc Close")).font(Tokens::ui(10.5)).color(t.text_dim));
        if t.glass {
            if let Some(shape) = app.backdrop.console_slice(ctx.content_rect(), ui.min_rect(), &app.session.prefs.appearance) {
                ui.painter().set(glass_shape, shape);
            }
            let rect = ui.min_rect();
            ui.painter().line_segment(
                [rect.left_top() - vec2(0.0, 9.0), rect.right_top() - vec2(0.0, 9.0)],
                Stroke::new(1.0, Color32::from_white_alpha((80.0 * t.reflections) as u8)),
            );
        }
    });
    app.quick_console = state;
    if close_now {
        close(app, ctx);
        return;
    }
    if let Some(key) = chosen {
        action = Some(("quickEffects.apply", json!({"key":key})));
    }
    if let Some((id, params)) = action {
        let result = if id == "prefs.open" {
            close(app, ctx);
            crate::menus::invoke(app, ctx, id, params)
        } else {
            command(app, ctx, id, params)
        };
        if let Err(error) = result {
            app.quick_console.error = error.clone();
            app.ui.status = error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preset_index_reads_only_names_and_extensions_and_never_parses_or_follows_symlinks() {
        let root = std::env::temp_dir().join(format!("cortez-console-scan-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("native.ecpreset"), b"not parsed").unwrap();
        std::fs::write(root.join("nested/preset.FFX"), b"not parsed").unwrap();
        std::fs::write(root.join("installer.exe"), b"not indexed").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let result = scan(&[root.to_string_lossy().into_owned()]);
        assert_eq!(result.files.len(), 2);
        assert!(result.files.iter().any(|file| file.kind == Kind::FfxPreset));
        std::fs::remove_dir_all(root).unwrap();
    }
}
