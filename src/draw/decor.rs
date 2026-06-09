//! Decorations drawn on top of the data: the legend (and, later, annotation
//! objects). Frame/title live in [`super::plot`].

use crate::draw::sets;
use crate::model::{Graph, LineType, SetType};
use crate::render::transform::WorldTransform;
use crate::render::{Canvas, HAlign, VAlign, VPoint};

/// Draw the legend if active and there is at least one labelled set.
pub fn draw_legend(canvas: &mut Canvas, graph: &Graph) {
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
        WorldTransform::new(graph)
            .world_to_view(l.x, l.y)
            .unwrap_or((l.x, l.y))
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

    // Box around the legend (filled then outlined), drawn first.
    if l.box_on {
        let pad_x = 0.01 * l.hgap;
        let pad_y = 0.01 * l.vgap;
        let content_h = entries.len() as f64 * em + (entries.len().saturating_sub(1)) as f64 * yskip;
        let x1 = ax - pad_x;
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

        // Swatch line.
        if set.line_type != LineType::None && set.linestyle != 0 {
            canvas.draw_polyline(
                &[VPoint { x: ax, y: y_mid }, VPoint { x: ax + ldist, y: y_mid }],
                set.line_pen.color,
                set.linewidth,
                set.linestyle,
            );
        }
        // Bar/fill sets without a line get a small filled box swatch.
        let is_bar = matches!(set.set_type, SetType::Bar | SetType::BarDy | SetType::BarDyDy);
        if is_bar || (set.line_type == LineType::None && set.symbol == crate::model::SymbolType::None) {
            let bw = 0.01 * max_symsize.max(0.8);
            let box_pts = [
                VPoint { x: ax + ldist / 2.0 - bw, y: y_mid - bw },
                VPoint { x: ax + ldist / 2.0 - bw, y: y_mid + bw },
                VPoint { x: ax + ldist / 2.0 + bw, y: y_mid + bw },
                VPoint { x: ax + ldist / 2.0 + bw, y: y_mid - bw },
            ];
            if set.symbol_fill.pattern != 0 {
                canvas.fill_polygon(&box_pts, set.symbol_fill.color, set.symbol_fill.pattern);
            }
            let mut closed = box_pts.to_vec();
            closed.push(box_pts[0]);
            canvas.draw_polyline(&closed, set.symbol_pen.color, set.symbol_linewidth, 1);
        }
        // Symbol at the swatch midpoint.
        sets::draw_symbol_at(canvas, set, VPoint { x: ax + ldist / 2.0, y: y_mid });

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
}
