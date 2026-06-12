//! Decorations drawn on top of the data: the legend (and, later, annotation
//! objects). Frame/title live in [`super::plot`].

use crate::draw::sets;
use crate::model::{Graph, LineType, SetType};
use crate::render::transform::WorldTransform;
use crate::render::{Canvas, ElementId, HAlign, VAlign, VPoint};

/// Draw the legend if active and there is at least one labelled set.
pub fn draw_legend(canvas: &mut Canvas, gno: usize, graph: &Graph) {
    let l = &graph.legend;
    if !l.active {
        return;
    }

    // Collect drawable, labelled sets in display order.
    let mut entries: Vec<&crate::model::Set> = graph
        .sets
        .iter()
        .filter(|s| !s.hidden && !s.legend.is_empty())
        .collect();
    if entries.is_empty() {
        return;
    }
    if l.invert {
        entries.reverse();
    }

    // Anchor (top-left) in view coordinates.
    let (ax, ay) = if l.loctype_view {
        (l.x, l.y)
    } else {
        WorldTransform::new(graph).world_to_view(l.x, l.y)
    };

    // Layout metrics (Grace: 0.01 * the respective legend parameters).
    let ldist = 0.01 * l.length; // swatch line length
    let max_symsize = entries.iter().map(|s| s.symbol_size).fold(0.0, f64::max);
    let sdist = 0.01 * (l.hgap + max_symsize); // gap before text
    let yskip = 0.01 * l.vgap;
    let em = canvas.em_view(l.charsize);
    let row = em + yskip;

    let text_x = ax + ldist + sdist;
    let max_text = entries
        .iter()
        .map(|s| canvas.text_width_view(&s.legend, l.charsize, l.font))
        .fold(0.0, f64::max);

    canvas.push_element(ElementId::Legend(gno));
    // The whole legend area is one clickable region, box or no box.
    {
        let content_h = entries.len() as f64 * em + (entries.len().saturating_sub(1)) as f64 * yskip;
        canvas.record_rect_view(ax, ay - content_h, text_x + max_text, ay);
    }

    // Box around the legend (filled then outlined), drawn first. Grace sizes
    // the box to enclose the actual content; with a short swatch line the
    // swatch can sit left of the anchor, so the box must extend to the
    // swatch's left edge, not just the anchor.
    if l.box_on {
        let pad_x = 0.01 * l.hgap;
        let pad_y = 0.01 * l.vgap;
        let content_h = entries.len() as f64 * em + (entries.len().saturating_sub(1)) as f64 * yskip;
        let max_hw = 0.01 * max_symsize;
        let any_line = entries
            .iter()
            .any(|s| ldist > 0.0 && s.line_type != LineType::None && s.linestyle != 0);
        let swatch_left = if any_line { ax - max_hw } else { ax + ldist / 2.0 - max_hw };
        let x1 = ax.min(swatch_left) - pad_x;
        let x2 = text_x + max_text + pad_x;
        let y_top = ay + pad_y;
        let y_bot = ay - content_h - pad_y;
        let rect = [
            VPoint { x: x1, y: y_bot },
            VPoint { x: x2, y: y_bot },
            VPoint { x: x2, y: y_top },
            VPoint { x: x1, y: y_top },
        ];
        if l.box_fill_pattern != 0 {
            canvas.fill_polygon(&rect, l.box_fill_color, l.box_fill_pattern);
        }
        if l.box_linestyle != 0 {
            let mut closed = rect.to_vec();
            closed.push(rect[0]);
            canvas.draw_polyline(&closed, l.box_color, l.box_linewidth, l.box_linestyle);
        }
    }

    // Entries top-down.
    let mut y_cur = ay;
    for set in entries {
        let y_mid = y_cur - em / 2.0;
        let vp = VPoint { x: ax, y: y_mid };
        let vp2 = VPoint { x: ax + ldist, y: y_mid };
        let vmid = VPoint { x: ax + ldist / 2.0, y: y_mid };
        let is_bar = matches!(set.set_type, SetType::Bar | SetType::BarDy | SetType::BarDyDy);

        // Grace's putlegends: with a line, draw it and place the swatch at both
        // ends; without a line, a single swatch at the midpoint. Bar/boxplot
        // sets use a rectangle swatch, others the set symbol.
        let has_line = ldist > 0.0 && set.line_type != LineType::None && set.linestyle != 0;
        if has_line {
            canvas.draw_polyline(&[vp, vp2], set.line_pen.color, set.linewidth, set.linestyle);
            if is_bar {
                draw_bar_swatch(canvas, set, vp, l.charsize);
                draw_bar_swatch(canvas, set, vp2, l.charsize);
            } else {
                sets::draw_symbol_at(canvas, set, vp);
                sets::draw_symbol_at(canvas, set, vp2);
            }
        } else if is_bar {
            draw_bar_swatch(canvas, set, vmid, l.charsize);
        } else {
            sets::draw_symbol_at(canvas, set, vmid);
        }

        // Label text, baseline-top aligned at the row top.
        canvas.draw_text(
            VPoint { x: text_x, y: y_cur },
            &set.legend,
            l.charsize,
            l.font,
            l.color,
            HAlign::Left,
            VAlign::Top,
            0.0,
        );

        y_cur -= row;
    }
    canvas.pop_element();
}

/// Draw a bar set's legend swatch (Grace `drawlegbarsym`): a rectangle
/// `0.02*symsize` wide × `0.02*legend_charsize` tall, filled with the symbol
/// fill pen and outlined with the symbol pen.
fn draw_bar_swatch(canvas: &mut Canvas, set: &crate::model::Set, center: VPoint, charsize: f64) {
    let hw = 0.01 * set.symbol_size;
    let hh = 0.01 * charsize;
    let rect = [
        VPoint { x: center.x - hw, y: center.y - hh },
        VPoint { x: center.x - hw, y: center.y + hh },
        VPoint { x: center.x + hw, y: center.y + hh },
        VPoint { x: center.x + hw, y: center.y - hh },
    ];
    if set.symbol_fill.pattern != 0 {
        canvas.fill_polygon(&rect, set.symbol_fill.color, set.symbol_fill.pattern);
    }
    if set.symbol_linestyle != 0 {
        let mut closed = rect.to_vec();
        closed.push(rect[0]);
        canvas.draw_polyline(&closed, set.symbol_pen.color, set.symbol_linewidth, set.symbol_linestyle);
    }
}
