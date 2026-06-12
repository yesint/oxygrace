//! Visual theme: high-contrast colors, enlarged fonts, dark/light modes
//! (View → Mode). Zoom/accent become configurable post-G5.

/// Accent used for selection highlights (tree, canvas overlay).
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 110, 255);
/// Halo painted under overlay strokes so they stay visible over any plot
/// content (white page, colored fills, dark lines alike).
pub const HALO: egui::Color32 = egui::Color32::WHITE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

/// User preference (View → Mode). egui reports the OS dark/light preference
/// (`Context::system_theme`, via winit) but not the system's actual palette
/// colors, so "System" picks between our own dark and light styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pref {
    System,
    Dark,
    Light,
}

/// Resolve a preference to a concrete mode using the OS preference.
pub fn resolve(ctx: &egui::Context, pref: Pref) -> Mode {
    match pref {
        Pref::Dark => Mode::Dark,
        Pref::Light => Mode::Light,
        Pref::System => match ctx.system_theme() {
            Some(egui::Theme::Light) => Mode::Light,
            _ => Mode::Dark,
        },
    }
}

/// Background of the canvas around the white plot page.
pub fn canvas_bg(mode: Mode) -> egui::Color32 {
    match mode {
        Mode::Dark => egui::Color32::from_gray(60),
        Mode::Light => egui::Color32::from_gray(165),
    }
}

pub fn apply(ctx: &egui::Context, mode: Mode) {
    // Larger UI all around (fonts, paddings, widgets).
    ctx.set_zoom_factor(1.25);
    let mut visuals = match mode {
        Mode::Dark => egui::Visuals::dark(),
        Mode::Light => egui::Visuals::light(),
    };
    match mode {
        Mode::Dark => {
            // Brighter text on the dark theme (defaults are mid-gray).
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_gray(230);
            visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_gray(230);
            visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
            visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
            visuals.widgets.open.fg_stroke.color = egui::Color32::WHITE;
            visuals.selection.bg_fill = egui::Color32::from_rgb(0, 84, 190);
            visuals.selection.stroke =
                egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 190, 255));
        }
        Mode::Light => {
            // Darker text on the light theme for the same crispness.
            visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_gray(20);
            visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_gray(20);
            visuals.widgets.hovered.fg_stroke.color = egui::Color32::BLACK;
            visuals.widgets.active.fg_stroke.color = egui::Color32::BLACK;
            visuals.widgets.open.fg_stroke.color = egui::Color32::BLACK;
            visuals.selection.bg_fill = egui::Color32::from_rgb(160, 205, 255);
            visuals.selection.stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 84, 190));
        }
    }
    visuals.hyperlink_color = ACCENT;
    ctx.global_style_mut(|style| {
        style.visuals = visuals.clone();
    });
}
