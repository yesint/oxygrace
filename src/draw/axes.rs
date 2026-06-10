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
/// Maximum number of ticks per axis before re-autoticking (Grace `MAX_TICKS`,
/// defines.h).
const MAX_TICKS: usize = 256;

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

    let scale = if is_x { graph.xscale } else { graph.yscale };
    let grid = tick_grid(
        wmin,
        wmax,
        scale,
        axis.major,
        axis.minor_ticks,
        axis.autonum,
        axis.tick_round,
    );
    let (majors, minors) = (grid.majors, grid.minors);

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

    // Majors that get a label: in the spec start/stop range, then every
    // (tl_skip+1)-th of those (drawticks.cpp: `itcur % (tl_skip + 1) == 0`,
    // where itcur runs over the in-range majors only).
    let labeled: Vec<f64> = majors
        .iter()
        .copied()
        .filter(|&t| tick_label_visible(axis, t))
        .enumerate()
        .filter(|(i, _)| *i as i32 % (axis.tl_skip + 1) == 0)
        .map(|(_, t)| t)
        .collect();

    if axis.ticklabels {
        for &t in &labeled {
            draw_tick_label(canvas, wt, &v, is_x, t, tl_base, axis);
        }
    }

    if !axis.label.is_empty() {
        // Grace places the axis label `tl_offset` beyond the tick-label bounding
        // box (graphs.cpp/drawticks.cpp: vp_label_offset = (vbase - bb_edge) +
        // tl_offset). The x label is TOP-justified (its top edge at the anchor);
        // the y label is MIDDLE-justified (centered on the anchor) — see
        // tlabel1_just. The visible gap then comes from that centering plus the
        // glyph side bearings.
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
        Some(VPoint { x: wt.x_to_view(t), y: v.ymin })
    } else {
        Some(VPoint { x: v.xmin, y: wt.y_to_view(t) })
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
        let vx = wt.x_to_view(t);
        canvas.draw_polyline(&[VPoint { x: vx, y: v.ymin }, VPoint { x: vx, y: v.ymax }], color, lw, ls);
    } else {
        let vy = wt.y_to_view(t);
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
        let vx = wt.x_to_view(t);
        let anchor = VPoint { x: vx, y: v.ymin - tl_base };
        canvas.draw_text(anchor, &label, axis.tl_charsize, axis.tl_font, axis.tl_color,
            HAlign::Center, VAlign::Top, axis.tl_angle as f64);
    } else {
        let vy = wt.y_to_view(t);
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
    let g = tick_grid(wmin, wmax, ScaleType::Normal, step, 0, 6, true);
    g.majors
}

/// Generated tick positions for one axis.
pub struct TickGrid {
    pub majors: Vec<f64>,
    pub minors: Vec<f64>,
}

/// `nicenum` (graphutils.cpp): a "nice" number approximately equal to `x`.
/// `round`: 0 = floor, 1 = ceil, 2 = round to 1/2/5/10.
fn nicenum(x: f64, nrange: i32, round: i32) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let xsign = x.signum();
    let x = x.abs();
    let fexp = x.log10().floor() - nrange as f64;
    let sx = x / 10f64.powf(fexp) / 10.0;
    let rx = sx.floor();
    let f = 10.0 * (sx - rx);
    let pos = xsign > 0.0;
    let y = match round {
        0 if pos => f.floor(),  // NICE_FLOOR, x > 0
        0 => f.ceil(),          // NICE_FLOOR, x < 0
        1 if pos => f.ceil(),   // NICE_CEIL, x > 0
        1 => f.floor(),         // NICE_CEIL, x < 0
        _ => {
            if f < 1.5 {
                1.0
            } else if f < 3.0 {
                2.0
            } else if f < 7.0 {
                5.0
            } else {
                10.0
            }
        }
    };
    xsign * (rx + y / 10.0) * 10.0 * 10f64.powf(fexp)
}

/// Generate major + minor tick positions, mirroring Grace's
/// `calculate_tickgrid` (drawticks.cpp, `TICKS_SPEC_NONE` branch).
///
/// On log axes both the world window and the major spacing live in log10
/// space: `tick major 10` means one major per decade, `tick major 2` one per
/// octave; the minor ticks are the `2..=nminor+1` multiples of each major.
/// On all other scales spacing is arithmetic. If the spacing is invalid or
/// would produce more than `MAX_TICKS` ticks, the spacing is recomputed like
/// Grace's `auto_ticks` (graphutils.cpp) from the axis' `autonum`.
pub fn tick_grid(
    wmin: f64,
    wmax: f64,
    scale: ScaleType,
    tmajor: f64,
    nminor: i32,
    autonum: i32,
    t_round: bool,
) -> TickGrid {
    let log = scale == ScaleType::Logarithmic;
    let (lo, hi) = (wmin.min(wmax), wmin.max(wmax));
    let (swc_lo, swc_hi) = if log {
        if hi <= 0.0 {
            return TickGrid { majors: vec![], minors: vec![] };
        }
        (lo.max(hi * 1e-30).log10(), hi.log10())
    } else {
        (lo, hi)
    };

    let mut tmajor = tmajor;
    let mut nminor = nminor;

    // auto_ticks (graphutils.cpp): pick a spacing giving about `autonum`
    // major ticks, and a sane minor count.
    let autotick = |tmajor: &mut f64, nminor: &mut i32| {
        let autonum = autonum.max(2) as f64;
        if log {
            if *tmajor <= 1.0 {
                *tmajor = 10.0;
            }
            let range = (swc_hi - swc_lo) / tmajor.log10();
            let d = (range / (autonum - 1.0)).ceil().max(1.0);
            *tmajor = tmajor.powf(d);
            if *nminor < 0 || *nminor > 10 {
                *nminor = 8;
            }
        } else {
            if *tmajor <= 0.0 {
                *tmajor = 1.0;
            }
            *tmajor = nicenum((swc_hi - swc_lo) / (autonum - 1.0), 0, 2);
            if *nminor < 0 || *nminor > 10 {
                *nminor = 1;
            }
        }
    };

    // Scaled spacing; invalid values trigger autoticking, as does an
    // excessive tick count (calculate_tickgrid's MAX_TICKS reenter loop).
    for attempt in 0..2 {
        let stmajor = if log { tmajor.log10() } else { tmajor };
        if stmajor <= 0.0 || !stmajor.is_finite() {
            autotick(&mut tmajor, &mut nminor);
            continue;
        }
        let mut swc_start = swc_lo;
        if t_round {
            swc_start = (swc_start / stmajor).floor() * stmajor;
        }
        let nmajor = ((swc_hi - swc_start) / stmajor + 1.0).ceil();
        let nticks = (nmajor - 1.0) * (nminor.max(0) as f64 + 1.0) + 1.0;
        if nmajor.is_nan() || nmajor < 1.0 || nticks > MAX_TICKS as f64 {
            if attempt == 0 {
                autotick(&mut tmajor, &mut nminor);
                continue;
            }
            return TickGrid { majors: vec![], minors: vec![] };
        }

        let mut majors = Vec::new();
        let mut minors = Vec::new();
        // Positions are generated over the rounded-down range and then
        // filtered to the world window, like Grace generates the full grid
        // and skips out-of-range ticks at draw time.
        let inside = |w: f64| {
            let tol = 1e-9 * (hi - lo).abs();
            w >= lo - tol && w <= hi + tol
        };
        for itmaj in 0..(nmajor as i64) {
            let s = swc_start + itmaj as f64 * stmajor;
            let wtmaj = if log {
                10f64.powf(s)
            } else if t_round && s.abs() < 1.0e-6 * stmajor {
                0.0
            } else {
                s
            };
            if inside(wtmaj) {
                majors.push(wtmaj);
            }
            for imtick in 0..nminor.max(0) {
                let w = if log {
                    // Minors at 2x, 3x … of the decade's major.
                    wtmaj * (imtick + 2) as f64
                } else {
                    wtmaj + (imtick + 1) as f64 * stmajor / (nminor as f64 + 1.0)
                };
                if inside(w) {
                    minors.push(w);
                }
            }
        }
        return TickGrid { majors, minors };
    }
    TickGrid { majors: vec![], minors: vec![] }
}

/// Format a tick value according to its format and precision, following
/// Grace's `create_fstring` (utils.cpp, `LFORMAT_TYPE_EXTENDED`): power and
/// scientific labels use text-markup superscripts, engineering uses SI
/// prefixes on multiples of 10³, computing uses K/M/G… on powers of 1024.
pub fn format_value(value: f64, format: TickFormat, prec: i32) -> String {
    let prec = prec.max(0) as usize;
    // Normalize negative zero so "-0.0" never appears.
    let v = if value == 0.0 { 0.0 } else { value };
    match format {
        TickFormat::Decimal => format!("{:.*}", prec, v),
        TickFormat::Exponential => format!("{:.*e}", prec, v),
        TickFormat::Scientific => {
            // "%.*fx10^%d" with the mantissa in [1, 10).
            if v == 0.0 {
                return format!("{:.*}", prec, 0.0);
            }
            let exponent = v.abs().log10().floor();
            let mantissa = v / 10f64.powf(exponent);
            format!("{:.*}\\x\\c4\\C\\f{{}}10\\S{}\\N", prec, mantissa, exponent as i64)
        }
        TickFormat::General => {
            // %g semantics: shortest of fixed/scientific with `prec`
            // significant digits, trailing zeros trimmed.
            let s = format!("{:.*}", prec.max(6), v);
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s.is_empty() { "0".to_string() } else { s.to_string() }
        }
        TickFormat::Power => {
            // 10^exp as markup; negative values as -10^exp (create_fstring
            // FORMAT_POWER). The exponent keeps `prec` decimals.
            if v == 0.0 {
                format!("{:.*}", prec, 0.0)
            } else if v < 0.0 {
                format!("-10\\S{:.*}\\N", prec, (-v).log10())
            } else {
                format!("10\\S{:.*}\\N", prec, v.log10())
            }
        }
        TickFormat::Engineering => {
            let exponent = if v != 0.0 {
                (v.abs().log10().floor().clamp(-24.0, 24.0) / 3.0).floor() * 3.0
            } else {
                0.0
            };
            let prefix = match exponent as i32 {
                -24 => "y",
                -21 => "z",
                -18 => "a",
                -15 => "f",
                -12 => "p",
                -9 => "n",
                // Micro is the Greek mu from the Symbol font.
                -6 => "\\xm\\f{}",
                -3 => "m",
                3 => "k",
                6 => "M",
                9 => "G",
                12 => "T",
                15 => "P",
                18 => "E",
                21 => "Z",
                24 => "Y",
                _ => "",
            };
            format!("{:.*} {}", prec, v / 10f64.powf(exponent), prefix)
        }
        TickFormat::Computing => {
            // Powers of 1024 with K/M/G… suffix (FORMAT_COMPUTING).
            let mut exponent = if v != 0.0 {
                let e = v.abs().log2().floor();
                if e < 10.0 {
                    0
                } else {
                    ((e / 10.0).floor() as i32 * 10).min(80)
                }
            } else {
                0
            };
            let sig = |x: f64, p: usize| -> String {
                // %.*g: p significant digits, trailing zeros trimmed.
                let s = format!("{:.*e}", p.saturating_sub(1), x);
                s.parse::<f64>().map(|f| {
                    let fs = format!("{}", f);
                    fs
                }).unwrap_or(s)
            };
            let mut s = sig(v / 2f64.powi(exponent), prec.max(1));
            // Roll to the next prefix for values that round to 1024.
            if exponent < 80 && s == "1024" {
                exponent += 10;
                s = sig(v / 2f64.powi(exponent), prec.max(1));
            }
            let prefix = match exponent {
                10 => "K",
                20 => "M",
                30 => "G",
                40 => "T",
                50 => "P",
                60 => "E",
                70 => "Z",
                80 => "Y",
                _ => "",
            };
            format!("{}{}", s, prefix)
        }
    }
}

/// True if a scale is logarithmic (used by callers deciding tick strategy).
pub fn is_log(scale: ScaleType) -> bool {
    matches!(scale, ScaleType::Logarithmic)
}
