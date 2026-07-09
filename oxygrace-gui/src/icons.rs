//! Toolbar icons from the embedded Phosphor symbolic icon font
//! (`install` adds it to egui's fonts once at startup). Icons are text
//! glyphs: tinted by the current foreground color, so they track the
//! dark/light theme, and rendered through the text pipeline, so they stay
//! crisp at any zoom/DPI.

#[derive(Clone, Copy)]
pub enum Icon {
    Open,
    Save,
    AutoscaleAll,
    AutoscaleSet,
    Pan,
    FreeAspect,
}

impl Icon {
    fn glyph(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Icon::Open => ph::FOLDER_OPEN,
            Icon::Save => ph::FLOPPY_DISK,
            Icon::AutoscaleAll => ph::ARROWS_OUT,
            Icon::AutoscaleSet => ph::MAGNIFYING_GLASS,
            Icon::Pan => ph::HAND,
            Icon::FreeAspect => ph::FRAME_CORNERS,
        }
    }
}

/// Register the Phosphor font with egui (call once, before the first frame).
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

/// A clickable, icon-only toolbar button. `active` shows the pressed/toggled
/// look (used for the modal Pan / Autoscale-to-set tools and Free aspect).
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tooltip: &str, active: bool) -> egui::Response {
    let text = egui::RichText::new(icon.glyph()).size(18.0);
    ui.add(egui::Button::new(text).min_size(egui::vec2(32.0, 30.0)).selected(active))
        .on_hover_text(tooltip)
}
