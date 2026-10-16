use effectcraft_ui_egui::EffectcraftApp;
use egui::{Event, pos2};
use egui_kittest::Harness;

/// The personalized page is longer than the old Appearance page. Scroll with real wheel input
/// until the setting is inside the body; never click an automation rectangle behind the footer.
pub fn scroll_to_setting(h: &mut Harness<'_, EffectcraftApp>, id: &str) {
    for _ in 0..24 {
        let element = h.state().auto.find(id).unwrap_or_else(|| panic!("no {id}")).clone();
        let nav = h.state().auto.find("settings.page.general").unwrap();
        let footer = h.state().auto.find("settings.cancel").unwrap();
        let top = nav.rect[1] + 30.0;
        let bottom = footer.rect[1] - 14.0;
        let [x, y, w, z] = element.rect;
        if y >= top && y + z <= bottom {
            return;
        }
        let center = (top + bottom) * 0.5;
        h.event(Event::PointerMoved(pos2(x + w * 0.5, center)));
        h.event(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, (center - y - z * 0.5).clamp(-500.0, 500.0)),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(12);
    }
    panic!("could not scroll {id} into the Settings body");
}
