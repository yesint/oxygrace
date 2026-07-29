//! The application's look: the two **style sheets** that define its dark and light themes.
//!
//! The styling itself is data — `themes/dark.toml` and `themes/light.toml`, each an egui preset
//! plus overrides, `include_str!`d into the binary and applied by the app-independent
//! [`egui_stylesheet`] crate (shared with molar_vis, where these sheets come from). This file is
//! only the glue: which sheet is which, and the handful of colors egui has no field for (see
//! [`Extras`]).
//!
//! egui keeps a separate `Style` per theme; both are configured on every [`apply`], and
//! [`set_theme`](egui::Context::set_theme) picks which one is live — so [`Pref::System`] follows
//! the host preference (OS dark/light, `prefers-color-scheme` in the browser) without us polling
//! it, and looks right either way.

use std::sync::LazyLock;

use egui::Color32;
use egui_stylesheet::StyleSheet;

/// Theme preference (View → Mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pref {
    /// Follow the host's dark/light preference. egui reports that preference
    /// (`Context::system_theme`, via winit) but not the system's actual palette colors, so
    /// "System" picks between *our* dark and light sheets.
    System,
    Dark,
    Light,
}

/// Apply the oxygrace look: configure both theme styles from the sheets and select one.
///
/// Idempotent and cheap (a sheet is parsed once per process; applying is a serde round trip of
/// `Visuals`), so it can be called at startup and again whenever the preference changes.
pub fn apply(ctx: &egui::Context, pref: Pref) {
    ctx.set_theme(match pref {
        Pref::System => egui::ThemePreference::System,
        Pref::Dark => egui::ThemePreference::Dark,
        Pref::Light => egui::ThemePreference::Light,
    });
    for (theme, sheet) in [(egui::Theme::Dark, &*DARK), (egui::Theme::Light, &*LIGHT)] {
        ctx.style_mut_of(theme, |style| {
            // `[metrics]` in the sheets are absolute sizes; a user font-scale setting would
            // multiply them here.
            if let Err(e) = sheet.apply(style, 1.0) {
                // The built-in sheets are validated by `built_in_sheets_apply`, so this can only
                // fire for a sheet loaded from elsewhere: keep the previous style and say so.
                log::error!("theme {}: {e}", sheet.name);
            }
        });
    }
}

/// The colors egui's `Visuals` has no field for. It models exactly two semantic colors —
/// `warn_fg_color` and `error_fg_color` — so anything else has to be named in a sheet's
/// `[extras]` table and looked up here.
///
/// Everything that *does* have an egui field goes through the style instead: a dimmed label reads
/// `ui.visuals().weak_text_color()`, not a helper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extras {
    /// Ink of the canvas selection/hover overlay (and of a selected swatch's border).
    pub accent: Color32,
    /// Halo painted under overlay strokes so they stay visible over any plot content (white page,
    /// colored fills, dark lines alike).
    pub halo: Color32,
    /// The desk around the plot page.
    pub canvas_bg: Color32,
}

impl Default for Extras {
    fn default() -> Self {
        Self {
            accent: Color32::from_rgb(0, 110, 255),
            halo: Color32::WHITE,
            canvas_bg: Color32::from_gray(60),
        }
    }
}

impl Extras {
    fn from_sheet(sheet: &StyleSheet) -> Self {
        let d = Self::default();
        Self {
            accent: sheet.extra("accent").unwrap_or(d.accent),
            halo: sheet.extra("halo").unwrap_or(d.halo),
            canvas_bg: sheet.extra("canvas_bg").unwrap_or(d.canvas_bg),
        }
    }
}

/// The live theme's extras. Keyed off `Context::theme` (the resolved dark/light choice), not off
/// the preference, so it is right in `System` mode too.
pub fn extras(ctx: &egui::Context) -> Extras {
    match ctx.theme() {
        egui::Theme::Dark => *DARK_EXTRAS,
        egui::Theme::Light => *LIGHT_EXTRAS,
    }
}

/// Accent used for selection highlights (canvas overlay, selected swatches).
pub fn accent(ctx: &egui::Context) -> Color32 {
    extras(ctx).accent
}

/// Background of the canvas around the plot page.
pub fn canvas_bg(ctx: &egui::Context) -> Color32 {
    extras(ctx).canvas_bg
}

/// The built-in sheets, parsed once per process (~20 µs each).
static DARK: LazyLock<StyleSheet> =
    LazyLock::new(|| load("dark", include_str!("../themes/dark.toml")));
static LIGHT: LazyLock<StyleSheet> =
    LazyLock::new(|| load("light", include_str!("../themes/light.toml")));
static DARK_EXTRAS: LazyLock<Extras> = LazyLock::new(|| Extras::from_sheet(&DARK));
static LIGHT_EXTRAS: LazyLock<Extras> = LazyLock::new(|| Extras::from_sheet(&LIGHT));

/// Parse a built-in sheet. A failure here is a bug in a file that ships *inside the binary*, and
/// `built_in_sheets_apply` catches it in CI — but a release build should still start, so it falls
/// back to the bare egui preset rather than panicking.
fn load(which: &str, src: &str) -> StyleSheet {
    match StyleSheet::parse(src) {
        Ok(s) => s,
        Err(e) => {
            log::error!("built-in {which} theme is invalid ({e}); falling back to egui's preset");
            StyleSheet::parse(&format!("name = \"fallback\"\nparent = \"{which}\"\n"))
                .expect("the minimal fallback sheet parses")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sheets ship *inside the binary*, so "does the theme file parse and apply" is a
    /// compile-and-test question, not something a user should discover at startup. Checks a few
    /// values from each file so a typo'd hex or a renamed egui field can't pass silently.
    #[test]
    fn built_in_sheets_apply() {
        let mut style = egui::Style::default();
        DARK.apply(&mut style, 1.0).expect("dark sheet");
        assert!(style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, Color32::from_rgb(20, 21, 25));
        // Text fields sit *above* the panel in both themes (egui's dark preset sinks them below).
        assert!(
            egui_stylesheet::luma(style.visuals.extreme_bg_color)
                > egui_stylesheet::luma(style.visuals.panel_fill)
        );
        let mut style = egui::Style::default();
        LIGHT.apply(&mut style, 1.0).expect("light sheet");
        assert!(!style.visuals.dark_mode);
        assert_eq!(style.visuals.panel_fill, Color32::from_rgb(198, 198, 198));
        assert_eq!(style.visuals.widgets.inactive.fg_stroke.color, Color32::BLACK);
        // A resting outline is what made frameless buttons resize on hover: it must stay off.
        assert_eq!(style.visuals.widgets.inactive.bg_stroke.width, 0.0);
        assert_eq!(style.visuals.error_fg_color, Color32::from_rgb(170, 30, 30));

        // Both themes cast a **dark** shadow, because a floating window is centered over the
        // white plot page — where the light bloom molar_vis uses would be invisible. Black,
        // premultiplied, so only the alpha channel carries anything.
        for sheet in [&*DARK, &*LIGHT] {
            let mut style = egui::Style::default();
            sheet.apply(&mut style, 1.0).expect("sheet");
            let sh = style.visuals.window_shadow.color;
            assert!(sh.a() > 0, "{}: must cast a shadow at all", sheet.name);
            let rgb = (sh.r(), sh.g(), sh.b());
            assert_eq!(rgb, (0, 0, 0), "{}: expected a dark shadow", sheet.name);
        }

        // Overlay ink is theme-independent (it is drawn over the plot's white paper); the desk
        // around the page is a UI surface, so it is not.
        assert_eq!(DARK_EXTRAS.accent, LIGHT_EXTRAS.accent);
        assert_eq!(DARK_EXTRAS.halo, LIGHT_EXTRAS.halo);
        assert!(
            egui_stylesheet::luma(DARK_EXTRAS.canvas_bg)
                < egui_stylesheet::luma(LIGHT_EXTRAS.canvas_bg)
        );
        // Both sheets name every extra (an unnamed one would silently fall back to a default).
        for sheet in [&*DARK, &*LIGHT] {
            for key in ["accent", "halo", "canvas_bg"] {
                assert!(sheet.extra(key).is_some(), "{}: missing extra {key}", sheet.name);
            }
        }
    }

    /// [`apply`] selects the requested style, and [`extras`] follows the *live* theme.
    #[test]
    fn apply_selects_the_theme() {
        let ctx = egui::Context::default();
        apply(&ctx, Pref::Light);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        assert_eq!(extras(&ctx), *LIGHT_EXTRAS);
        assert!(!ctx.global_style().visuals.dark_mode);
        apply(&ctx, Pref::Dark);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert_eq!(extras(&ctx), *DARK_EXTRAS);
        assert!(ctx.global_style().visuals.dark_mode);
        // Both styles are configured on every apply, so switching needs no re-apply.
        assert_eq!(
            ctx.style_of(egui::Theme::Light).visuals.panel_fill,
            Color32::from_rgb(198, 198, 198)
        );
    }

    /// Hovering a widget must not **resize** it — in either theme.
    ///
    /// egui's `Style::button_style` subtracts the resting `bg_stroke.width` from `inner_margin`
    /// so that adding a border doesn't change a *framed* button's size. But a button that is
    /// frameless at rest (`Button::selectable` — the toolbar's tool toggles, and every menu row)
    /// drops the stroke and keeps the shrunken margin, so a resting border of width 1 makes those
    /// widgets **1 px smaller at rest than on hover**: the row twitches under the cursor.
    #[test]
    fn hover_does_not_resize_widgets() {
        use std::cell::RefCell;
        for pref in [Pref::Dark, Pref::Light] {
            let ctx = egui::Context::default();
            apply(&ctx, pref);
            let seen: RefCell<Vec<(&str, egui::Rect)>> = RefCell::new(Vec::new());
            // One frame with the pointer at `pointer`; yields each probe widget's rect.
            let frame = |pointer: egui::Pos2| {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    events: vec![egui::Event::PointerMoved(pointer)],
                    ..Default::default()
                };
                seen.borrow_mut().clear();
                let _ = ctx.run_ui(input, |ui| {
                    let r = ui.button("Button").rect;
                    seen.borrow_mut().push(("button", r));
                    // Frameless at rest — the case that breaks.
                    let r = ui.selectable_label(false, "Toggle").rect;
                    seen.borrow_mut().push(("selectable_label", r));
                    let r = ui.menu_button("Menu", |_| {}).response.rect;
                    seen.borrow_mut().push(("menu_button", r));
                });
                seen.borrow().clone()
            };
            // egui reads a widget's state from the *previous* frame's response, so every
            // measurement is the second of two identical frames.
            let corner = egui::pos2(399.0, 299.0);
            frame(corner);
            let rest = frame(corner);
            for (i, (name, rect)) in rest.iter().enumerate() {
                frame(rect.center());
                let hovered = frame(rect.center())[i].1;
                assert_eq!(
                    rect.size(),
                    hovered.size(),
                    "{pref:?}: {name} resizes on hover ({:?} → {:?})",
                    rect.size(),
                    hovered.size()
                );
            }
        }
    }
}
