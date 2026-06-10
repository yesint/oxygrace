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
    // Specified ticks (TICKS_SPEC_MARKS/BOTH) replace the generated grid;
    // out-of-window positions are skipped like in Grace's draw loops.
    let (majors, minors) = if axis.spec_type != 0 {
        let n = if axis.spec_count > 0 {
            axis.spec_count.min(axis.spec_ticks.len())
        } else {
            axis.spec_ticks.len()
        };
        let (lo, hi) = (wmin.min(wmax), wmin.max(wmax));
        let inside = |p: f64| p >= lo - 1e-9 * (hi - lo) && p <= hi + 1e-9 * (hi - lo);
        let mut majors = Vec::new();
        let mut minors = Vec::new();
        for t in &axis.spec_ticks[..n] {
            if inside(t.pos) {
                if t.major {
                    majors.push(t.pos);
                } else {
                    minors.push(t.pos);
                }
            }
        }
        (majors, minors)
    } else {
        let grid = tick_grid(
            wmin,
            wmax,
            scale,
            axis.major,
            axis.minor_ticks,
            axis.autonum,
            axis.tick_round,
        );
        (grid.majors, grid.minors)
    };

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

    // --- Axis geometry (drawticks.cpp drawaxes): every axis has a normal
    // side (vbase1, at the lower/left world edge) and an opposite side
    // (vbase2); `axis offset` pushes them outward. A zero axis puts both at
    // world 0 of the perpendicular coordinate and flips the tick direction
    // (tick_dir_sign = -1); it is skipped when 0 is outside the window.
    let (vbase1, vbase2, sign) = if axis.zero {
        let (pmin, pmax) = if is_x {
            (graph.world.ymin, graph.world.ymax)
        } else {
            (graph.world.xmin, graph.world.xmax)
        };
        if !(pmin.min(pmax) <= 0.0 && 0.0 <= pmin.max(pmax)) {
            return;
        }
        let v0 = if is_x { wt.y_to_view(0.0) } else { wt.x_to_view(0.0) };
        (v0 - axis.offs_normal, v0 + axis.offs_opposite, -1.0)
    } else {
        let (e1, e2) = if is_x { (v.ymin, v.ymax) } else { (v.xmin, v.xmax) };
        (e1 - axis.offs_normal, e2 + axis.offs_opposite, 1.0)
    };
    // Extent along the axis and a (along, perpendicular) -> view point helper.
    let (amin, amax) = if is_x { (v.xmin, v.xmax) } else { (v.ymin, v.ymax) };
    let pt = |along: f64, perp: f64| {
        if is_x {
            VPoint { x: along, y: perp }
        } else {
            VPoint { x: perp, y: along }
        }
    };
    let along_of = |t: f64| if is_x { wt.x_to_view(t) } else { wt.y_to_view(t) };
    let on_normal = axis.op != 1;
    let on_opposite = axis.op != 0;

    // Axis bar on the chosen side(s) (t_drawbar / t_op).
    if axis.draw_bar {
        if on_normal {
            canvas.draw_polyline(
                &[pt(amin, vbase1), pt(amax, vbase1)],
                axis.bar_color,
                axis.bar_linewidth,
                axis.bar_linestyle,
            );
        }
        if on_opposite {
            canvas.draw_polyline(
                &[pt(amin, vbase2), pt(amax, vbase2)],
                axis.bar_color,
                axis.bar_linewidth,
                axis.bar_linestyle,
            );
        }
    }

    // Tick marks: start/stop per side from the in/out/both switch
    // (drawticks.cpp t_inout; the zero-axis sign flips the direction).
    if axis.ticks {
        for (list, props) in [(&minors, &axis.minor_props), (&majors, &axis.major_props)] {
            let tsize = TICK_UNIT * props.size;
            let (s1a, s1b, s2a, s2b) = match axis.tick_inout {
                0 => (vbase1, vbase1 + sign * tsize, vbase2, vbase2 - sign * tsize),
                1 => (vbase1, vbase1 - sign * tsize, vbase2, vbase2 + sign * tsize),
                _ => (vbase1 - tsize, vbase1 + tsize, vbase2 + tsize, vbase2 - tsize),
            };
            for &t in list.iter() {
                let a = along_of(t);
                if on_normal {
                    canvas.draw_polyline(&[pt(a, s1a), pt(a, s1b)], props.color, props.linewidth, props.linestyle);
                }
                if on_opposite {
                    canvas.draw_polyline(&[pt(a, s2a), pt(a, s2b)], props.color, props.linewidth, props.linestyle);
                }
            }
        }
    }

    // Tick label baselines per side (drawticks.cpp vbase_tlabel1/2).
    let tsize = TICK_UNIT * axis.major_props.size;
    let (tl1, tl2) = match axis.tick_inout {
        0 => (
            vbase1 - (1.0 - sign) / 2.0 * tsize - TL_OFFSET,
            vbase2 + (1.0 - sign) / 2.0 * tsize + TL_OFFSET,
        ),
        1 => (
            vbase1 - (1.0 + sign) / 2.0 * tsize - TL_OFFSET,
            vbase2 + (1.0 + sign) / 2.0 * tsize + TL_OFFSET,
        ),
        _ => (vbase1 - tsize - TL_OFFSET, vbase2 + tsize + TL_OFFSET),
    };

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

    let tl_on_normal = axis.tl_op != 1;
    let tl_on_opposite = axis.tl_op != 0;
    if axis.ticklabels {
        for &t in &labeled {
            let a = along_of(t);
            let text = tick_label_text(axis, t);
            // Normal side: x labels hang below (CENTER|TOP), y labels sit
            // left (RIGHT|MIDDLE); the opposite side mirrors both.
            if tl_on_normal {
                let (ha, va) = if is_x {
                    (HAlign::Center, VAlign::Top)
                } else {
                    (HAlign::Right, VAlign::Middle)
                };
                canvas.draw_text(pt(a, tl1), &text, axis.tl_charsize, axis.tl_font, axis.tl_color, ha, va, axis.tl_angle as f64);
            }
            if tl_on_opposite {
                let (ha, va) = if is_x {
                    (HAlign::Center, VAlign::Bottom)
                } else {
                    (HAlign::Left, VAlign::Middle)
                };
                canvas.draw_text(pt(a, tl2), &text, axis.tl_charsize, axis.tl_font, axis.tl_color, ha, va, axis.tl_angle as f64);
            }
        }
    }

    // Axis label: tl_offset beyond the side's bounding box of tick marks and
    // tick labels (vp_label_offset = (vbase - bb_edge) + tl_offset).
    if !axis.label.is_empty() {
        // Outward tick extents beyond each base (mirror of the tick switch).
        let tick_ext = match axis.tick_inout {
            0 => (1.0 - sign) / 2.0 * tsize,
            1 => (1.0 + sign) / 2.0 * tsize,
            _ => tsize,
        };
        let tick_ext = if axis.ticks { tick_ext } else { 0.0 };
        let ink = if axis.ticklabels && !labeled.is_empty() {
            tick_label_extent(canvas, is_x, &labeled, axis)
        } else {
            0.0
        };
        let mid = (amin + amax) / 2.0;
        if axis.label_op != 1 {
            let lab_ext = if ink > 0.0 && tl_on_normal {
                (vbase1 - tl1) + ink
            } else {
                0.0
            };
            let offset = tick_ext.max(lab_ext) + TL_OFFSET;
            let anchor = pt(mid, vbase1 - offset);
            let (ha, va) = if is_x {
                (HAlign::Center, VAlign::Top)
            } else {
                (HAlign::Right, VAlign::Middle)
            };
            let angle = if is_x { 0.0 } else { 90.0 };
            canvas.draw_text(anchor, &axis.label, axis.label_charsize, axis.label_font, axis.label_color, ha, va, angle);
        }
        if axis.label_op != 0 {
            let lab_ext = if ink > 0.0 && tl_on_opposite {
                (tl2 - vbase2) + ink
            } else {
                0.0
            };
            let offset = tick_ext.max(lab_ext) + TL_OFFSET;
            let anchor = pt(mid, vbase2 + offset);
            let (ha, va) = if is_x {
                (HAlign::Center, VAlign::Bottom)
            } else {
                (HAlign::Left, VAlign::Middle)
            };
            let angle = if is_x { 0.0 } else { 90.0 };
            canvas.draw_text(anchor, &axis.label, axis.label_charsize, axis.label_font, axis.label_color, ha, va, angle);
        }
    }
}

/// Text of the label at major tick `t`: the specified label when the axis
/// uses TICKS_SPEC_BOTH, otherwise the formatted value with pre/append.
fn tick_label_text(axis: &Axis, t: f64) -> String {
    if axis.spec_type == 2 {
        let hit = axis
            .spec_ticks
            .iter()
            .find(|s| (s.pos - t).abs() <= 1e-9 * (1.0 + t.abs()));
        if let Some(l) = hit.and_then(|s| s.label.as_ref()) {
            return l.clone();
        }
    }
    // The formula transforms the value before formatting (drawticks.cpp
    // evaluates tl_formula over the major tick positions with $t bound).
    let t = if axis.tl_formula.is_empty() {
        t
    } else {
        crate::parse::formula::eval(&axis.tl_formula, t).unwrap_or(t)
    };
    format!(
        "{}{}{}",
        axis.tl_prepend,
        format_value(t, axis.tl_format, axis.tl_prec),
        axis.tl_append
    )
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
            let s = tick_label_text(axis, t);
            let (x0, y0, x1, y1) = canvas.text_bbox_view(&s, axis.tl_charsize, axis.tl_font);
            if is_x {
                y1 - y0
            } else {
                x1 - x0
            }
        })
        .fold(0.0, f64::max)
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
