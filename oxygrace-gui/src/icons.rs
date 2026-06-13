//! Toolbar icons drawn procedurally with the egui painter — monochrome
//! line/fill art, compiled into the binary (no image assets). Each icon is
//! laid out in the unit square of its button and tinted with the current
//! foreground color, so it tracks the dark/light theme.

#[derive(Clone, Copy)]
pub enum Icon {
    Open,
    Save,
    AutoscaleAll,
    AutoscaleSet,
    Pan,
    FreeAspect,
}

/// A clickable, icon-only toolbar button. `active` shows the pressed/toggled
/// look (used for the modal Pan / Autoscale-to-set tools and Free aspect).
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, tooltip: &str, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(32.0, 30.0), egui::Sense::click());
    let v = ui.style().interact_selectable(&resp, active);
    ui.painter()
        .rect(rect, 4.0, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
    // `bg` is used by the Pan icon to cut finger notches that read cleanly
    // at any size.
    draw(ui.painter(), icon, rect.shrink(7.0), v.fg_stroke.color, v.weak_bg_fill);
    resp.on_hover_text(tooltip)
}

/// Draw `icon` inside `r` (the icon area) in `color`, with `bg` matching the
/// button fill (for cut-out details).
fn draw(p: &egui::Painter, icon: Icon, r: egui::Rect, color: egui::Color32, bg: egui::Color32) {
    let w = (r.width().min(r.height()) * 0.13).max(1.5); // stroke width
    let stroke = egui::Stroke::new(w, color);
    // Fractional point inside the icon area.
    let at = |fx: f32, fy: f32| r.lerp_inside(egui::vec2(fx, fy));
    let line = |a: egui::Pos2, b: egui::Pos2| p.line_segment([a, b], stroke);
    let poly = |pts: Vec<egui::Pos2>| {
        p.add(egui::Shape::closed_line(pts, stroke));
    };

    match icon {
        // A folder.
        Icon::Open => poly(vec![
            at(0.08, 0.82),
            at(0.08, 0.30),
            at(0.40, 0.30),
            at(0.50, 0.42),
            at(0.92, 0.42),
            at(0.92, 0.82),
        ]),

        // A floppy disk: body with a folded top-right corner, the metal
        // shutter, and the label.
        Icon::Save => {
            poly(vec![
                at(0.12, 0.12),
                at(0.74, 0.12),
                at(0.88, 0.26),
                at(0.88, 0.88),
                at(0.12, 0.88),
            ]);
            // Shutter (top), with a slot.
            poly(vec![at(0.34, 0.12), at(0.66, 0.12), at(0.66, 0.34), at(0.34, 0.34)]);
            line(at(0.58, 0.16), at(0.58, 0.30));
            // Label (bottom).
            poly(vec![at(0.28, 0.52), at(0.72, 0.52), at(0.72, 0.88), at(0.28, 0.88)]);
        }

        // Four arrows pointing out to the corners ("fit / expand to all").
        Icon::AutoscaleAll => {
            for &(cx, cy, sx, sy) in &[
                (0.5, 0.5, -1.0, -1.0),
                (0.5, 0.5, 1.0, -1.0),
                (0.5, 0.5, -1.0, 1.0),
                (0.5, 0.5, 1.0, 1.0),
            ] {
                let tip = at(cx + sx * 0.42, cy + sy * 0.42);
                let tail = at(cx + sx * 0.12, cy + sy * 0.12);
                line(tail, tip);
                // Arrowhead at the tip (two short barbs toward the centre).
                line(tip, at(cx + sx * 0.42 - sx * 0.20, cy + sy * 0.42));
                line(tip, at(cx + sx * 0.42, cy + sy * 0.42 - sy * 0.20));
            }
        }

        // A magnifier ("zoom to a chosen set").
        Icon::AutoscaleSet => {
            let c = at(0.40, 0.40);
            let rad = r.width() * 0.26;
            p.circle_stroke(c, rad, stroke);
            line(at(0.60, 0.60), at(0.92, 0.92));
        }

        // An open "grab" hand: a rounded palm-and-fingers blob with the
        // finger gaps and thumb-split cut out in the button color, so the
        // fingers read even at toolbar size.
        Icon::Pan => {
            let round = r.width() * 0.16;
            // Palm + fingers as one rounded blob.
            p.rect_filled(
                egui::Rect::from_two_pos(at(0.28, 0.18), at(0.76, 0.90)),
                round,
                color,
            );
            // Three notches splitting the top into four fingers.
            let notch = |cx: f32| {
                p.rect_filled(
                    egui::Rect::from_two_pos(
                        at(cx - 0.025, 0.10),
                        at(cx + 0.025, 0.52),
                    ),
                    0.0,
                    bg,
                );
            };
            notch(0.40);
            notch(0.52);
            notch(0.64);
            // Split the thumb off the lower-left of the palm, then re-add a
            // rounded thumb tip poking out to the left.
            p.rect_filled(
                egui::Rect::from_two_pos(at(0.12, 0.40), at(0.30, 0.50)),
                0.0,
                bg,
            );
            p.rect_filled(
                egui::Rect::from_two_pos(at(0.14, 0.50), at(0.30, 0.66)),
                round,
                color,
            );
        }

        // A frame with a double-headed diagonal arrow ("free resize / aspect").
        Icon::FreeAspect => {
            poly(vec![at(0.10, 0.10), at(0.90, 0.10), at(0.90, 0.90), at(0.10, 0.90)]);
            let a = at(0.30, 0.70);
            let b = at(0.70, 0.30);
            line(a, b);
            line(a, at(0.30, 0.52));
            line(a, at(0.48, 0.70));
            line(b, at(0.70, 0.48));
            line(b, at(0.52, 0.30));
        }
    }
}
