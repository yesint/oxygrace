//! Pie charts (plotone.cpp `draw_pie_chart`).
//!
//! A pie graph draws exactly one set (types xy / xycolor / xycolpat):
//! column x holds the slice values, y the explode factors, the optional
//! third/fourth columns per-slice colors and fill patterns. Pie graphs draw
//! no axes and no legend; the frame, objects and titles come from the
//! normal per-graph passes.

use crate::model::{Graph, SetType};
use crate::render::{Canvas, HAlign, VAlign, VPoint};

/// Sample step for arcs, in radians (about 1.5 degrees — smooth under AA).
const ARC_STEP: f64 = 0.025;

pub fn draw_pie(canvas: &mut Canvas, graph: &Graph) {
    let v = graph.view;
    let vpc = VPoint {
        x: (v.xmin + v.xmax) / 2.0,
        y: (v.ymin + v.ymax) / 2.0,
    };
    let sgn = if graph.xinvert { -1.0 } else { 1.0 };

    // Grace draws only the first drawable set of a pie.
    let Some(set) = graph.sets.iter().find(|s| {
        !s.hidden
            && !s.data.is_empty()
            && matches!(s.set_type, SetType::Xy | SetType::XyColor | SetType::XyColPat)
    }) else {
        return;
    };
    let cols = &set.data.cols;
    let (Some(xs), Some(es)) = (cols.first(), cols.get(1)) else {
        return;
    };
    let colors = cols.get(2);
    let patterns = cols.get(3);
    let n = xs.len().min(es.len());
    if n == 0 {
        return;
    }

    // Radius shrinks to keep the most exploded slice inside 80% of the
    // viewport half-extent (plotone.cpp: r = 0.8/(1+e_max)*MIN2(w,h)/2).
    let e_max = es[..n].iter().copied().fold(0.0f64, f64::max);
    let r = 0.8 / (1.0 + e_max) * (v.xmax - v.xmin).min(v.ymax - v.ymin) / 2.0;
    let norm: f64 = xs[..n].iter().sum();
    if norm <= 0.0 || xs[..n].iter().any(|&x| x < 0.0) {
        return;
    }

    let mut stop_angle = graph.world.xmin;
    for i in 0..n {
        let start_angle = stop_angle;
        stop_angle = start_angle + sgn * 2.0 * std::f64::consts::PI * xs[i] / norm;
        let mid = (start_angle + stop_angle) / 2.0;
        let off = VPoint {
            x: es[i] * r * mid.cos(),
            y: es[i] * r * mid.sin(),
        };
        let c = VPoint {
            x: vpc.x + off.x,
            y: vpc.y + off.y,
        };

        // The slice outline: center -> arc -> back to center.
        let mut pts = vec![c];
        let steps = ((stop_angle - start_angle).abs() / ARC_STEP).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let a = start_angle + (stop_angle - start_angle) * k as f64 / steps as f64;
            pts.push(VPoint {
                x: c.x + r * a.cos(),
                y: c.y + r * a.sin(),
            });
        }

        // Fill: per-slice color/pattern columns (rounded) with the symbol
        // fill pen as the default.
        let color = match colors.and_then(|c| c.get(i)) {
            Some(&ci) => ci.round() as i32,
            None => set.symbol_fill.color,
        };
        let pattern = match patterns.and_then(|p| p.get(i)) {
            Some(&pi) => pi.round() as i32,
            None => set.symbol_fill.pattern,
        };
        canvas.fill_polygon(&pts, color, pattern);

        // Outline (radius, arc, radius) with the symbol pen.
        if set.symbol_linestyle != 0 {
            let mut closed = pts.clone();
            closed.push(c);
            canvas.draw_polyline(&closed, set.symbol_pen.color, set.symbol_linewidth, set.symbol_linestyle);
        }

        // Slice labels sit just outside the arc on its bisector.
        let av = &set.avalue;
        if av.active {
            let rad = (1.0 + es[i]) * r + av.offy;
            let anchor = VPoint {
                x: vpc.x + rad * mid.cos(),
                y: vpc.y + rad * mid.sin(),
            };
            let value = match av.avtype {
                1 => crate::draw::axes::format_value(xs[i], av.format, av.prec),
                4 => match set.data.strs.get(i).and_then(|s| s.clone()) {
                    Some(s) => s,
                    None => continue,
                },
                _ => continue,
            };
            let text = format!("{}{}{}", av.prepend, value, av.append);
            canvas.draw_text(
                anchor,
                &text,
                av.size,
                av.font,
                av.color,
                HAlign::Center,
                VAlign::Middle,
                av.angle,
            );
        }
    }
}
