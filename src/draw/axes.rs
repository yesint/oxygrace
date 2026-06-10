//! Axis drawing: bars, major/minor tick marks, grid lines, numeric tick
//! labels and axis labels.
//!
//! Milestone 1 covers linear (and basic logarithmic) scales with an explicit
//! `tick major` spacing; the full autotick algorithm and the date/geographic
//! tick-label formats are deferred.

use crate::model::{Axis, AxisId, Graph, ScaleType, TickFormat};
use crate::render::{Canvas, HAlign, VAlign, VPoint, WorldTransform};

/// View-units length of a unit-size tick (Grace's `0.02 * size`).
const TICK_UNIT: f64 = 0.02;
/// Perpendicular gap (view units) between a tick and its label, and between
/// the tick labels and the axis label (Grace's auto `tl_offset`).
const TL_OFFSET: f64 = 0.01;
/// Maximum number of ticks generated for one axis (runaway guard).
const MAX_TICKS: usize = 1000;

/// Draw all active axes of a graph.
pub fn draw_axes(canvas: &mut Canvas, graph: &Graph) {
    let wt = WorldTransform::new(graph);
    for id in [AxisId::X, AxisId::Y, AxisId::AltX, AxisId::AltY] {
        let axis = &graph.axes[id.index()];
        if axis.active {
            draw_one_axis(canvas, graph, &wt, id, axis);
        }
    }
}

fn draw_one_axis(canvas: &mut Canvas, graph: &Graph, wt: &WorldTransform, id: AxisId, axis: &Axis) {
    let v = graph.view;
    let is_x = id.is_x();
    // Position of this axis along the perpendicular direction (the frame edge
    // it sits on). For the primary X/Y axes that is the bottom / left edge.
    let (wmin, wmax) = if is_x {
        (graph.world.xmin, graph.world.xmax)
    } else {
        (graph.world.ymin, graph.world.ymax)
    };

    let majors = major_ticks(wmin, wmax, axis.major);
    let minors = minor_ticks(&majors, axis.minor_ticks, wmin, wmax);

    // Grid lines first so ticks/data sit on top.
    if axis.major_props.grid {
        for &t in &majors {
            draw_grid_line(canvas, wt, &v, is_x, t, axis.major_props.color, axis.major_props.linewidth, axis.major_props.linestyle);
        }
    }
    if axis.minor_props.grid {
        for &t in &minors {
            draw_grid_line(canvas, wt, &v, is_x, t, axis.minor_props.color, axis.minor_props.linewidth, axis.minor_props.linestyle);
        }
    }

    // Axis bar along the frame edge.
    if axis.draw_bar {
        let (a, b) = if is_x {
            (VPoint { x: v.xmin, y: v.ymin }, VPoint { x: v.xmax, y: v.ymin })
        } else {
            (VPoint { x: v.xmin, y: v.ymin }, VPoint { x: v.xmin, y: v.ymax })
        };
        canvas.draw_polyline(&[a, b], axis.bar_color, axis.bar_linewidth, axis.bar_linestyle);
    }

    if axis.ticks {
        let sign = if axis.ticks_in { 1.0 } else { -1.0 };
        // Minor then major ticks (both on the two opposite edges, as Grace's
        // default `tick op both`).
        for &t in &minors {
            draw_tick(canvas, wt, &v, is_x, t, TICK_UNIT * axis.minor_props.size * sign,
                axis.minor_props.color, axis.minor_props.linewidth, axis.minor_props.linestyle);
        }
        for &t in &majors {
            draw_tick(canvas, wt, &v, is_x, t, TICK_UNIT * axis.major_props.size * sign,
                axis.major_props.color, axis.major_props.linewidth, axis.major_props.linestyle);
        }
    }

    // Perpendicular offset from the axis to the tick-label anchor. Inward
    // ticks don't extend outside the frame, so only the gap applies; outward
    // ticks also clear the tick length (Grace's vbase_tlabel computation).
    let tsize = TICK_UNIT * axis.major_props.size;
    let tl_base = if axis.ticks_in { 0.0 } else { tsize } + TL_OFFSET;

    if axis.ticklabels {
        for &t in &majors {
            if tick_label_visible(axis, t) {
                draw_tick_label(canvas, wt, &v, is_x, t, tl_base, axis);
            }
        }
    }

    if !axis.label.is_empty() {
        // Grace places the axis label `tl_offset` beyond the tick-label bounding
        // box (graphs.cpp/drawticks.cpp: vp_label_offset = (vbase - bb_edge) +
        // tl_offset). The x label is TOP-justified (its top edge at the anchor);
        // the y label is MIDDLE-justified (centered on the anchor) — see
        // tlabel1_just. The visible gap then comes from that centering plus the
        // glyph side bearings.
        let labeled: Vec<f64> = majors
            .iter()
            .copied()
            .filter(|&t| tick_label_visible(axis, t))
            .collect();
        let tl_extent = if axis.ticklabels {
            tick_label_extent(canvas, is_x, &labeled, axis)
        } else {
            0.0
        };
        let offset = tl_base + tl_extent + TL_OFFSET;
        draw_axis_label(canvas, &v, is_x, axis, offset);
    }
}

/// Whether a tick at world value `t` gets a label, honoring spec start/stop
/// bounds (Grace `drawticks.cpp`: skip if `t < tl_start` or `t > tl_stop`),
/// with a small tolerance so the boundary ticks themselves are kept.
fn tick_label_visible(axis: &Axis, t: f64) -> bool {
    let tol = |v: f64| 1e-6 * (1.0 + v.abs());
    if axis.tl_start_spec && t < axis.tl_start - tol(axis.tl_start) {
        return false;
    }
    if axis.tl_stop_spec && t > axis.tl_stop + tol(axis.tl_stop) {
        return false;
    }
    true
}

/// Perpendicular extent of the rendered tick labels in view units — the max
/// ink height for x-axes (labels stack below), the max ink width for y-axes
/// (labels extend left). This is the tick-label side of Grace's `bb`.
fn tick_label_extent(canvas: &Canvas, is_x: bool, majors: &[f64], axis: &Axis) -> f64 {
    majors
        .iter()
        .map(|&t| {
            let s = format!(
                "{}{}{}",
                axis.tl_prepend,
                format_value(t, axis.tl_format, axis.tl_prec),
                axis.tl_append
            );
            let (x0, y0, x1, y1) = canvas.text_bbox_view(&s, axis.tl_charsize, axis.tl_font);
            if is_x {
                y1 - y0
            } else {
                x1 - x0
            }
        })
        .fold(0.0, f64::max)
}

/// Map a tick value to its view position on the axis (returns view x and y).
fn tick_view_pos(wt: &WorldTransform, v: &crate::model::View, is_x: bool, t: f64) -> Option<VPoint> {
    if is_x {
        let vx = wt.x_to_view(t)?;
        Some(VPoint { x: vx, y: v.ymin })
    } else {
        let vy = wt.y_to_view(t)?;
        Some(VPoint { x: v.xmin, y: vy })
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tick(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    v: &crate::model::View,
    is_x: bool,
    t: f64,
    len: f64,
    color: i32,
    lw: f64,
    ls: i32,
) {
    let Some(base) = tick_view_pos(wt, v, is_x, t) else { return };
    if is_x {
        // Bottom edge tick (inward = up) and top edge tick (inward = down).
        let bottom = [base, VPoint { x: base.x, y: base.y + len }];
        let top = [VPoint { x: base.x, y: v.ymax }, VPoint { x: base.x, y: v.ymax - len }];
        canvas.draw_polyline(&bottom, color, lw, ls);
        canvas.draw_polyline(&top, color, lw, ls);
    } else {
        let left = [base, VPoint { x: base.x + len, y: base.y }];
        let right = [VPoint { x: v.xmax, y: base.y }, VPoint { x: v.xmax - len, y: base.y }];
        canvas.draw_polyline(&left, color, lw, ls);
        canvas.draw_polyline(&right, color, lw, ls);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_grid_line(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    v: &crate::model::View,
    is_x: bool,
    t: f64,
    color: i32,
    lw: f64,
    ls: i32,
) {
    if is_x {
        let Some(vx) = wt.x_to_view(t) else { return };
        canvas.draw_polyline(&[VPoint { x: vx, y: v.ymin }, VPoint { x: vx, y: v.ymax }], color, lw, ls);
    } else {
        let Some(vy) = wt.y_to_view(t) else { return };
        canvas.draw_polyline(&[VPoint { x: v.xmin, y: vy }, VPoint { x: v.xmax, y: vy }], color, lw, ls);
    }
}

fn draw_tick_label(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    v: &crate::model::View,
    is_x: bool,
    t: f64,
    tl_base: f64,
    axis: &Axis,
) {
    let label = format!("{}{}{}", axis.tl_prepend, format_value(t, axis.tl_format, axis.tl_prec), axis.tl_append);
    if is_x {
        let Some(vx) = wt.x_to_view(t) else { return };
        let anchor = VPoint { x: vx, y: v.ymin - tl_base };
        canvas.draw_text(anchor, &label, axis.tl_charsize, axis.tl_font, axis.tl_color,
            HAlign::Center, VAlign::Top, axis.tl_angle as f64);
    } else {
        let Some(vy) = wt.y_to_view(t) else { return };
        let anchor = VPoint { x: v.xmin - tl_base, y: vy };
        canvas.draw_text(anchor, &label, axis.tl_charsize, axis.tl_font, axis.tl_color,
            HAlign::Right, VAlign::Middle, axis.tl_angle as f64);
    }
}

/// Draw the axis label at perpendicular distance `offset` from the axis,
/// centered along it. Mirrors Grace's `drawticks.cpp`: the x label is drawn
/// `JUST_CENTER|JUST_TOP` at angle 0, the y label `JUST_RIGHT|JUST_MIDDLE` at
/// angle 90 (for the default parallel layout). `draw_text` positions each label
/// by its rendered bounding box, so the placement follows the real glyph
/// extents — no per-string constants.
fn draw_axis_label(canvas: &mut Canvas, v: &crate::model::View, is_x: bool, axis: &Axis, offset: f64) {
    if is_x {
        let anchor = VPoint { x: (v.xmin + v.xmax) / 2.0, y: v.ymin - offset };
        canvas.draw_text(anchor, &axis.label, axis.label_charsize, axis.label_font, axis.label_color,
            HAlign::Center, VAlign::Top, 0.0);
    } else {
        let anchor = VPoint { x: v.xmin - offset, y: (v.ymin + v.ymax) / 2.0 };
        canvas.draw_text(anchor, &axis.label, axis.label_charsize, axis.label_font, axis.label_color,
            HAlign::Right, VAlign::Middle, 90.0);
    }
}

/// Generate major tick world positions at multiples of `step`.
pub fn major_ticks(wmin: f64, wmax: f64, step: f64) -> Vec<f64> {
    let mut out = Vec::new();
    if !step.is_finite() || step <= 0.0 {
        return out;
    }
    let (lo, hi) = (wmin.min(wmax), wmin.max(wmax));
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    if last < first || (last - first) as usize > MAX_TICKS {
        return out;
    }
    for n in first..=last {
        let pos = n as f64 * step;
        // Guard against floating point landing just outside the window.
        if pos >= lo - step * 1e-9 && pos <= hi + step * 1e-9 {
            out.push(pos);
        }
    }
    out
}

/// Generate minor tick positions between majors (excluding major positions).
fn minor_ticks(majors: &[f64], nminor: i32, wmin: f64, wmax: f64) -> Vec<f64> {
    let mut out = Vec::new();
    if nminor <= 0 || majors.is_empty() {
        return out;
    }
    // Spacing inferred from consecutive majors; fall back to none if unknown.
    let step = if majors.len() >= 2 {
        majors[1] - majors[0]
    } else {
        return out;
    };
    let minor_step = step / (nminor as f64 + 1.0);
    let (lo, hi) = (wmin.min(wmax), wmin.max(wmax));
    // Extend one major step below the first major to cover the leading edge.
    let start = majors[0] - step;
    let mut x = start;
    let mut guard = 0usize;
    while x <= hi + step && guard < MAX_TICKS {
        guard += 1;
        x += minor_step;
        if x < lo || x > hi {
            continue;
        }
        // Skip positions that coincide with a major tick.
        if majors.iter().any(|&m| (m - x).abs() < minor_step * 1e-6) {
            continue;
        }
        out.push(x);
    }
    out
}

/// Format a tick value according to its format and precision.
pub fn format_value(value: f64, format: TickFormat, prec: i32) -> String {
    let prec = prec.max(0) as usize;
    // Normalize negative zero so "-0.0" never appears.
    let v = if value == 0.0 { 0.0 } else { value };
    match format {
        TickFormat::Decimal => format!("{:.*}", prec, v),
        TickFormat::Exponential | TickFormat::Scientific => format!("{:.*e}", prec, v),
        TickFormat::General => {
            // Trim trailing zeros from a decimal representation.
            let s = format!("{:.*}", prec.max(6), v);
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s.is_empty() { "0".to_string() } else { s.to_string() }
        }
        TickFormat::Power | TickFormat::Engineering => format!("{:.*}", prec, v),
    }
}

/// True if a scale is logarithmic (used by callers deciding tick strategy).
pub fn is_log(scale: ScaleType) -> bool {
    matches!(scale, ScaleType::Logarithmic)
}
