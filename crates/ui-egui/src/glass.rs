//! CortezEffects UI surfaces. Wallpaper decoding/blur runs once on a bounded background job,
//! never in the compositor. No backdrop sampling, video readback or platform-specific unsafe API.

use egui::{Color32, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::theme::Tokens;

#[derive(Default)]
pub(crate) struct Backdrop {
    key: Option<(String, u32)>,
    texture: Option<egui::TextureHandle>,
    pending: Option<std::sync::mpsc::Receiver<Result<egui::ColorImage, String>>>,
}

impl Backdrop {
    /// Reuse the already blurred wallpaper texture; never read back the viewport for a popup.
    pub(crate) fn console_slice(&self, viewport: Rect, rect: Rect, a: &effectcraft_engine::prefs::Appearance) -> Option<egui::Shape> {
        let texture = self.texture.as_ref()?;
        let size = texture.size().map(|value| value.max(1) as f32);
        let (target, uv) = wallpaper_rect(viewport, vec2(size[0], size[1]), &a.wallpaper_fit);
        let visible = rect.intersect(target);
        if !visible.is_positive() || !target.is_positive() {
            return None;
        }
        let low = uv.min + ((visible.min - target.min) / target.size()) * uv.size();
        let high = uv.min + ((visible.max - target.min) / target.size()) * uv.size();
        let mut mesh = egui::Mesh::with_texture(texture.id());
        mesh.add_rect_with_uv(visible, Rect::from_min_max(low, high), Color32::WHITE.gamma_multiply((a.wallpaper_opacity as f32 * 0.18).clamp(0.0, 0.18)));
        Some(egui::Shape::mesh(mesh))
    }

    pub fn paint(&mut self, ui: &egui::Ui, rect: Rect, a: &effectcraft_engine::prefs::Appearance, t: &Tokens, busy: bool) -> Option<String> {
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, t.app_bg);
        let effects = t.glass && !(busy && a.adaptive_effects);
        if effects {
            crate::theme::gradient_rect(&painter, rect, t.app_bg.lerp_to_gamma(t.accent, 0.10), t.app_bg);
        }
        // Quantize slider changes to avoid spawning a job for every fractional pointer movement.
        let blur = if effects { (a.wallpaper_blur + a.glass_blur).round().clamp(0.0, 48.0) as u32 } else { 0 };
        let key = (a.wallpaper_path.clone(), blur);
        let mut error = None;
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok(img) => self.texture = Some(ui.ctx().load_texture("cortezeffects-wallpaper", img, egui::TextureOptions::LINEAR)),
                        Err(e) => error = Some(e),
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    error = Some("Wallpaper worker stopped".into());
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        // Never re-decode because playback toggles: keep the cached image and just hide expensive
        // decoration while busy. A requested new image waits for the one outstanding job.
        if self.pending.is_none() && self.key.as_ref().is_none_or(|old| old.0 != key.0 || (!busy && old.1 != key.1)) {
            self.key = Some(key.clone());
            self.texture = None;
            if !key.0.is_empty() {
                let (tx, rx) = std::sync::mpsc::channel();
                self.pending = Some(rx);
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let ctx = ui.ctx().clone();
                    if let Err(e) = std::thread::Builder::new().name("cortez-wallpaper".into()).spawn(move || {
                        let result = decode_wallpaper(&key.0, key.1 as f32);
                        let _ = tx.send(result);
                        ctx.request_repaint();
                    }) {
                        self.pending = None;
                        error = Some(format!("Wallpaper: {e}"));
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = tx.send(Err("Local wallpaper requires the desktop application".into()));
                }
            }
        }
        if self.pending.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(tex) = &self.texture {
            let [w, h] = tex.size().map(|n| n.max(1) as f32);
            let (target, uv) = wallpaper_rect(rect, vec2(w, h), &a.wallpaper_fit);
            painter.image(tex.id(), target, uv, Color32::WHITE.gamma_multiply(a.wallpaper_opacity as f32));
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha((255.0 * a.wallpaper_dim) as u8));
        }
        error
    }
}

/// Aspect-correct contain/cover. Geometry only; no per-frame image resampling.
pub(crate) fn wallpaper_rect(rect: Rect, size: egui::Vec2, fit: &str) -> (Rect, Rect) {
    let full_uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    if !rect.is_positive() || size.x <= 0.0 || size.y <= 0.0 {
        return (rect, full_uv);
    }
    if fit == "contain" {
        let scale = (rect.width() / size.x).min(rect.height() / size.y);
        (Rect::from_center_size(rect.center(), size * scale), full_uv)
    } else if fit == "stretch" {
        (rect, full_uv)
    } else {
        let scale = (rect.width() / size.x).max(rect.height() / size.y);
        let uv_size = rect.size() / (size * scale);
        (rect, Rect::from_center_size(pos2(0.5, 0.5), uv_size))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_wallpaper(path: &str, blur: f32) -> Result<egui::ColorImage, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| format!("Wallpaper: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Wallpaper must be a regular image file".into());
    }
    let mut bytes = Vec::new();
    file.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| format!("Wallpaper: {e}"))?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Wallpaper exceeds 32 MiB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| format!("Wallpaper: {e}"))?;
    let image = if image.width() > 1280 || image.height() > 1280 { image.thumbnail(1280, 1280) } else { image }.to_rgba8();
    let image = if blur > 0.0 { image::imageops::blur(&image, blur.min(48.0)) } else { image };
    Ok(egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw()))
}

/// Simulated glass reflections and edge light, with a constant number of cheap primitives.
pub(crate) fn surface(p: &egui::Painter, rect: Rect, t: &Tokens) {
    if t.shadows > 0.0 {
        p.rect_filled(rect.translate(vec2(0.0, 2.0)).expand(1.0), t.radius + 1.0, Color32::from_black_alpha((45.0 * t.shadows) as u8));
    }
    p.rect_filled(rect, t.radius, t.panel_bg);
    if t.glass {
        p.rect_stroke(rect.shrink(0.5), t.radius, Stroke::new(0.7, t.accent.gamma_multiply(0.16 + t.glow * 0.22)), StrokeKind::Inside);
        let y = rect.top() + 1.0;
        p.line_segment(
            [pos2(rect.left() + t.radius, y), pos2(rect.right() - t.radius, y)],
            Stroke::new(1.0, Color32::from_white_alpha((70.0 * t.reflections) as u8)),
        );
    }
}

/// Real font/pipeline preparation uses the existing startup frames; there is no timer/delay.
pub(crate) fn splash(ui: &mut egui::Ui, t: &Tokens) {
    let r = ui.max_rect();
    crate::theme::gradient_rect(ui.painter(), r, t.app_bg.lerp_to_gamma(t.accent, 0.18), t.app_bg);
    let c = r.center();
    crate::header::paint_logo(ui.painter(), Rect::from_center_size(c - vec2(0.0, 50.0), vec2(128.0, 128.0)));
    ui.painter().text(c + vec2(0.0, 44.0), egui::Align2::CENTER_CENTER, "CortezEffects", egui::FontId::proportional(30.0), t.tab_text_active);
    ui.painter().text(c + vec2(0.0, 80.0), egui::Align2::CENTER_CENTER, crate::i18n::tr("Preparing editor…"), Tokens::ui(12.0), t.text_dim);
    if t.motion > 0.0 {
        ui.put(Rect::from_center_size(c + vec2(0.0, 116.0), vec2(18.0, 18.0)), egui::Spinner::new().color(t.accent));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_renders_before_named_application_fonts_are_ready() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 600.0))), ..Default::default() }, |ui| {
            splash(ui, &Tokens::for_kind(crate::theme::ThemeKind::Dark))
        });
        assert!(!output.shapes.is_empty());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn wallpaper_decodes_blurs_and_rejects_invalid_input() {
        let dir = std::env::temp_dir().join(format!("cortez-wallpaper-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("generated.png");
        let img = image::RgbaImage::from_fn(32, 16, |x, _| if x < 16 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        img.save(&path).unwrap();
        let clear = decode_wallpaper(path.to_str().unwrap(), 0.0).unwrap();
        let blurred = decode_wallpaper(path.to_str().unwrap(), 8.0).unwrap();
        assert_eq!(clear.size, [32, 16]);
        assert_ne!(clear.pixels, blurred.pixels, "blur control must change the cached background");
        let mut backdrop = Backdrop::default();
        let appearance = effectcraft_engine::prefs::Appearance {
            wallpaper_path: path.to_string_lossy().into_owned(),
            glass_blur: 0.0,
            wallpaper_blur: 0.0,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while backdrop.texture.is_none() && std::time::Instant::now() < deadline {
            let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 480.0))), ..Default::default() }, |ui| {
                assert!(backdrop.paint(ui, ui.max_rect(), &appearance, &Tokens::for_kind(crate::theme::ThemeKind::Dark), false).is_none());
            });
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(backdrop.texture.is_some(), "background worker must upload an actual cached wallpaper texture");
        std::fs::write(&path, b"not a PNG").unwrap();
        assert!(decode_wallpaper(path.to_str().unwrap(), 0.0).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wallpaper_fit_preserves_aspect_and_crops_symmetrically() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 200.0));
        let (target, uv) = wallpaper_rect(rect, vec2(400.0, 200.0), "cover");
        assert_eq!(target, rect);
        assert_eq!(uv, Rect::from_min_max(pos2(0.25, 0.0), pos2(0.75, 1.0)));
        let (target, uv) = wallpaper_rect(rect, vec2(400.0, 200.0), "contain");
        assert_eq!(target.size(), vec2(200.0, 100.0));
        assert_eq!(target.center(), rect.center());
        assert_eq!(uv.size(), vec2(1.0, 1.0));
    }
}
