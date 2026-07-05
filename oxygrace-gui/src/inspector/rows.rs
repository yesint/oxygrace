//! The property-row vocabulary: `label | control` rows shared by every
//! inspector page, all queueing [`Edit`]s — so pages look and behave
//! identically.
//!
//! Rows are **elastic**: the label column has a fixed width and the control
//! fills the remaining panel width, so the panel can be resized freely (a
//! fixed-width control would set the panel's minimum width — egui panels
//! never shrink below their content).
//!
//! Each function takes the current value from the immutable model, an undo
//! label, and a setter closure; `live` change events (mid-drag, mid-typing)
//! coalesce into one undo step.

use oxygrace::Project;

use crate::edit::Edit;

/// Fixed width of the label column.
const LABEL_WIDTH: f32 = 115.0;

const LINESTYLE_NAMES: [&str; 9] = [
    "None",
    "Solid",
    "Dotted",
    "Dashed",
    "Long dash",
    "Dot-dash",
    "Dot-long dash",
    "Dot-dash-dot",
    "Dash-dot-dash",
];

/// One `label | control` row: a fixed-width, vertically centered,
/// truncating label, then the control in the remaining width. Every row
/// goes through this, so all pages share one rhythm.
fn row<R>(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let size = egui::vec2(LABEL_WIDTH, ui.spacing().interact_size.y);
        ui.allocate_ui_with_layout(
            size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(size);
                ui.add(egui::Label::new(label).truncate());
            },
        );
        control(ui)
    })
    .inner
}

/// Elastic control width: the remaining row width minus the item gap, so
/// the control fills the panel without setting a minimum panel width.
fn control_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() - ui.spacing().item_spacing.x).max(40.0)
}

/// A small −/+ stepper button; returns true when clicked.
fn step_button(ui: &mut egui::Ui, glyph: &str) -> bool {
    ui.add(egui::Button::new(glyph).min_size(egui::vec2(20.0, 18.0)))
        .clicked()
}

/// The shared − / drag-or-type / + spin control behind [`num`] and [`int`]:
/// returns `Some((new value, live))` when edited (`live` = mid-gesture, so
/// successive arrow clicks / drag frames coalesce into one undo step).
fn spin<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    label: &str,
    v: T,
    speed: f64,
    step: f64,
    decimals: usize,
    range: Option<std::ops::RangeInclusive<T>>,
) -> Option<(T, bool)> {
    row(ui, label, |ui| {
        let mut val = v;
        let mut changed = false;
        let mut live = false;
        let clamp = |x: f64| match &range {
            Some(r) => x.clamp(r.start().to_f64(), r.end().to_f64()),
            None => x,
        };
        if step_button(ui, "−") {
            val = T::from_f64(clamp(v.to_f64() - step));
            changed |= val != v;
            live = true;
        }
        let mut dv = egui::DragValue::new(&mut val).speed(speed).max_decimals(decimals);
        if let Some(r) = &range {
            dv = dv.range(r.clone());
        }
        let resp = ui.add(dv);
        if resp.changed() {
            changed = true;
            live = resp.dragged();
        }
        if step_button(ui, "+") {
            val = T::from_f64(clamp(v.to_f64() + step));
            changed |= val != v;
            live = true;
        }
        changed.then_some((val, live))
    })
}

/// A float spin box: − / drag-or-type / + (arrows step by 10× the drag
/// speed; scrolling and dragging the value still work).
pub fn num(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: f64,
    speed: f64,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, f64) + 'static,
) {
    if let Some((val, live)) = spin(ui, label, v, speed, speed * 10.0, 6, None) {
        edits.push(Edit::new(ulabel, val, live, set));
    }
}

/// An integer spin box, clamped to `range` (arrows step by 1).
pub fn int(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: i32,
    range: std::ops::RangeInclusive<i32>,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, i32) + 'static,
) {
    if let Some((val, live)) = spin(ui, label, v, 0.1, 1.0, 0, Some(range)) {
        edits.push(Edit::new(ulabel, val, live, set));
    }
}

/// A single-line text field; typing coalesces into one undo step.
pub fn text(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: &str,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, String) + 'static,
) {
    let mut val = v.to_owned();
    row(ui, label, |ui| {
        let resp = ui.add(egui::TextEdit::singleline(&mut val).desired_width(f32::INFINITY));
        if resp.changed() {
            edits.push(Edit::new(ulabel, val, resp.has_focus(), set));
        }
    });
}

/// An on/off checkbox.
pub fn toggle(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: bool,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, bool) + 'static,
) {
    let mut val = v;
    row(ui, label, |ui| {
        if ui.checkbox(&mut val, "").changed() {
            edits.push(Edit::new(ulabel, val, false, set));
        }
    });
}

/// A generic enum/value combo over `(value, name)` options.
pub fn combo<T: Copy + PartialEq + 'static>(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: T,
    options: &[(T, &str)],
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, T) + 'static,
) {
    row(ui, label, |ui| {
        let current = options
            .iter()
            .find(|(val, _)| *val == v)
            .map(|(_, name)| *name)
            .unwrap_or("?");
        let mut picked = None;
        egui::ComboBox::from_id_salt((ui.id(), label))
            .width(control_width(ui))
            .selected_text(current)
            .show_ui(ui, |ui| {
                for &(val, name) in options {
                    if ui.selectable_label(val == v, name).clicked() {
                        picked = Some(val);
                    }
                }
            });
        if let Some(val) = picked {
            edits.push(Edit::new(ulabel, val, false, set));
        }
    });
}

/// A boxed value setter (so optional setters can be passed around).
type Setter = Box<dyn FnOnce(&mut Project, i32)>;

/// A Grace color index: a swatch button that drops down a grid of
/// clickable swatches (names only on hover — no numbers in the UI).
pub fn color(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: i32,
    project: &Project,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, i32) + 'static,
) {
    color_impl(ui, edits, label, v, project, ulabel, Box::new(set), None);
}

/// A color row whose picker also carries an opacity slider (0..=255, the
/// QtGrace per-pen alpha channel).
#[allow(clippy::too_many_arguments)]
pub fn color_alpha(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: i32,
    alpha: i32,
    project: &Project,
    ulabel: &'static str,
    alabel: &'static str,
    set: impl FnOnce(&mut Project, i32) + 'static,
    set_alpha: impl FnOnce(&mut Project, i32) + 'static,
) {
    color_impl(
        ui,
        edits,
        label,
        v,
        project,
        ulabel,
        Box::new(set),
        Some((alpha, alabel, Box::new(set_alpha))),
    );
}

#[allow(clippy::too_many_arguments)]
fn color_impl(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: i32,
    project: &Project,
    ulabel: &'static str,
    set: Setter,
    alpha: Option<(i32, &'static str, Setter)>,
) {
    row(ui, label, |ui| {
        let rgba = resolve(project, v);
        // The button is the current color itself, with a contrast arrow.
        let arrow = if luminance(rgba) > 0.5 {
            egui::Color32::BLACK
        } else {
            egui::Color32::WHITE
        };
        let resp = ui.add(
            egui::Button::new(egui::RichText::new("⏷").color(arrow).size(11.0))
                .fill(rgba)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(140)))
                .min_size(egui::vec2(48.0, 18.0)),
        );
        let mut picked = None;
        // Close only on an outside click so the opacity slider is usable;
        // a swatch pick closes explicitly.
        egui::Popup::menu(&resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                // 16 built-ins plus any override indices beyond them (built
                // only while the popup is open).
                let mut indices: Vec<i32> = (0..16).collect();
                for &(i, _) in &project.color_overrides {
                    if !(0..16).contains(&i) {
                        indices.push(i);
                    }
                }
                indices.sort_unstable();
                indices.dedup();
                egui::Grid::new("color_grid").spacing([4.0, 4.0]).show(ui, |ui| {
                    for (n, &i) in indices.iter().enumerate() {
                        let c = resolve(project, i);
                        let current = i == v;
                        let stroke = if current {
                            egui::Stroke::new(2.0, crate::theme::ACCENT)
                        } else {
                            egui::Stroke::new(1.0, egui::Color32::from_gray(120))
                        };
                        let b = ui
                            .add(
                                egui::Button::new("")
                                    .fill(c)
                                    .stroke(stroke)
                                    .min_size(egui::vec2(24.0, 24.0)),
                            )
                            .on_hover_text(color_name(project, i));
                        if b.clicked() {
                            picked = Some(i);
                            ui.close();
                        }
                        if n % 8 == 7 {
                            ui.end_row();
                        }
                    }
                });
                // Opacity slider (QtGrace pen alpha, 0..=255); slider drags
                // coalesce into one undo step.
                if let Some((a, alabel, set_alpha)) = alpha {
                    ui.separator();
                    let mut val = a;
                    let sr = ui.add(
                        egui::Slider::new(&mut val, 0..=255)
                            .text("Opacity")
                            .show_value(true),
                    );
                    if sr.changed() {
                        edits.push(Edit::new(alabel, val, sr.dragged(), set_alpha));
                    }
                }
            });
        if let Some(val) = picked {
            edits.push(Edit::new(ulabel, val, false, set));
        }
    });
}

fn resolve(project: &Project, index: i32) -> egui::Color32 {
    let c = oxygrace::color::resolve(project, index);
    egui::Color32::from_rgb(c.r, c.g, c.b)
}

fn luminance(c: egui::Color32) -> f32 {
    (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) / 255.0
}

fn color_name(project: &Project, index: i32) -> String {
    const NAMES: [&str; 16] = [
        "white", "black", "red", "green", "blue", "yellow", "brown", "grey", "violet", "cyan",
        "magenta", "orange", "indigo", "maroon", "turquoise", "green4",
    ];
    let overridden = project.color_overrides.iter().any(|&(i, _)| i == index);
    match usize::try_from(index) {
        Ok(i) if i < 16 && !overridden => NAMES[i].to_string(),
        _ => format!("custom {index}"),
    }
}

/// A Grace fill pattern (0 none, 1 solid, 2..=31 hatches): a sample button
/// dropping down a grid of pattern samples.
pub fn pattern(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: i32,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, i32) + 'static,
) {
    row(ui, label, |ui| {
        let resp = pattern_sample(ui, v, egui::vec2(48.0, 18.0), false)
            .on_hover_text(pattern_name(v));
        let mut picked = None;
        egui::Popup::menu(&resp).show(|ui| {
            egui::Grid::new("pattern_grid").spacing([4.0, 4.0]).show(ui, |ui| {
                let n = oxygrace::patterns::PATTERN_BITS.len() as i32;
                for i in 0..n {
                    let r = pattern_sample(ui, i, egui::vec2(28.0, 28.0), i == v)
                        .on_hover_text(pattern_name(i));
                    if r.clicked() {
                        picked = Some(i);
                    }
                    if i % 8 == 7 {
                        ui.end_row();
                    }
                }
            });
        });
        if let Some(val) = picked {
            edits.push(Edit::new(ulabel, val, false, set));
        }
    });
}

/// Paint one pattern sample (the 16×16 Grace tile repeated to fill `size`).
fn pattern_sample(ui: &mut egui::Ui, idx: i32, size: egui::Vec2, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, egui::Color32::WHITE);
    if idx == 0 {
        // "None": an empty sample with a red slash.
        p.line_segment(
            [rect.left_bottom(), rect.right_top()],
            egui::Stroke::new(1.5, egui::Color32::from_rgb(200, 60, 60)),
        );
    } else if let Some(bits) = oxygrace::patterns::PATTERN_BITS.get(idx as usize) {
        let px = (size.y / 16.0).max(1.0);
        let cols = (size.x / px).ceil() as usize;
        let rows = (size.y / px).ceil() as usize;
        for row in 0..rows {
            for col in 0..cols {
                // X11 bitmap order: LSB-first within each byte (canvas.rs).
                let byte = bits[(row % 16) * 2 + (col % 16) / 8];
                if (byte >> (col % 8)) & 1 == 1 {
                    let min = egui::pos2(rect.left() + col as f32 * px, rect.top() + row as f32 * px);
                    p.rect_filled(
                        egui::Rect::from_min_size(min, egui::vec2(px, px)).intersect(rect),
                        0.0,
                        egui::Color32::BLACK,
                    );
                }
            }
        }
    }
    let stroke = if selected {
        egui::Stroke::new(2.0, crate::theme::ACCENT)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_gray(120))
    };
    p.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
    resp
}

fn pattern_name(idx: i32) -> String {
    match idx {
        0 => "None".into(),
        1 => "Solid".into(),
        _ => format!("Pattern {idx}"),
    }
}

/// A Grace font slot: combo over the 14 slots with their mapped face names.
pub fn font(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: i32,
    project: &Project,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, i32) + 'static,
) {
    let name = |slot: i32| -> String {
        let face = project
            .font_map
            .get(slot.clamp(0, 13) as usize)
            .copied()
            .unwrap_or(0)
            .clamp(0, 13) as usize;
        format!("{slot} — {}", oxygrace::font::FACE_NAMES[face])
    };
    row(ui, label, |ui| {
        let mut picked = None;
        egui::ComboBox::from_id_salt((ui.id(), label))
            .width(control_width(ui))
            .selected_text(name(v))
            .show_ui(ui, |ui| {
                for slot in 0..14 {
                    if ui.selectable_label(slot == v, name(slot)).clicked() {
                        picked = Some(slot);
                    }
                }
            });
        if let Some(val) = picked {
            edits.push(Edit::new(ulabel, val, false, set));
        }
    });
}

/// A Grace line style (0..=8) with a painted dash preview per option.
pub fn linestyle(
    ui: &mut egui::Ui,
    edits: &mut Vec<Edit>,
    label: &str,
    v: i32,
    ulabel: &'static str,
    set: impl FnOnce(&mut Project, i32) + 'static,
) {
    row(ui, label, |ui| {
        let mut picked = None;
        let current = LINESTYLE_NAMES.get(v.max(0) as usize).copied().unwrap_or("?");
        egui::ComboBox::from_id_salt((ui.id(), label))
            .width(control_width(ui))
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (i, name) in LINESTYLE_NAMES.iter().enumerate() {
                    ui.horizontal(|ui| {
                        dash_preview(ui, i as i32);
                        if ui.selectable_label(i as i32 == v, *name).clicked() {
                            picked = Some(i as i32);
                        }
                    });
                }
            });
        if let Some(val) = picked {
            edits.push(Edit::new(ulabel, val, false, set));
        }
    });
}

/// Paint a small sample line in Grace's dash pattern for the style, driven
/// by the renderer's own dash table (`DASH_PATTERNS`, width-relative runs).
fn dash_preview(ui: &mut egui::Ui, style: i32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 14.0), egui::Sense::hover());
    if style == 0 {
        return; // none: draw nothing
    }
    let y = rect.center().y;
    let stroke = egui::Stroke::new(2.0, ui.visuals().text_color());
    let dashes = oxygrace::render::canvas::DASH_PATTERNS
        .get(style.max(0) as usize)
        .copied()
        .unwrap_or(&[]);
    if dashes.is_empty() {
        // Solid.
        ui.painter()
            .line_segment([rect.left_center(), rect.right_center()], stroke);
        return;
    }
    // Runs are in line widths; preview at a 2 px line width.
    let unit = 2.0;
    let mut x = rect.left();
    let mut draw = true;
    let mut i = 0;
    while x < rect.right() {
        let seg = (dashes[i % dashes.len()] * unit).min(rect.right() - x);
        if draw {
            ui.painter()
                .line_segment([egui::pos2(x, y), egui::pos2(x + seg, y)], stroke);
        }
        x += seg;
        draw = !draw;
        i += 1;
    }
}

/// A section header within a page (collapsible, all pages share the look).
///
/// `force`: `Some(open)` overrides the stored collapse state this frame —
/// used when the selection changes, so the relevant section expands and the
/// others fold away; `None` leaves it interactive.
pub fn section(
    ui: &mut egui::Ui,
    title: &str,
    default_open: bool,
    force: Option<bool>,
    id: &str,
    content: impl FnOnce(&mut egui::Ui),
) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong())
        .id_salt(id)
        .default_open(default_open)
        .open(force)
        .show(ui, content);
}
