//! Dataset drawing. Milestone 1 draws the connecting line of XY-like sets;
//! symbols, fills and error bars are added in later milestones.

use crate::model::{FillType, GraphType, Graph, LineType, Set, SetType, SymbolType};
use crate::render::{Canvas, ElementId, VPoint, WorldTransform};

/// True for bar dataset types.
fn is_bar(t: SetType) -> bool {
    matches!(t, SetType::Bar | SetType::BarDy | SetType::BarDyDy)
}

/// 1/sqrt(3), used for equilateral-triangle symbol vertices (matches Grace).
const SQRT1_3: f64 = 0.577_350_269_189_625_8;
/// 1/sqrt(2), used for the diagonal "x" symbol.
const SQRT1_2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Draw every visible set of a graph: connecting line, then symbols.
///
/// Clipping mirrors Grace's `plotone.cpp`: fills, lines, droplines and bars
/// are drawn with clipping to the graph viewport (`setclipping(TRUE)`), while
/// symbols are drawn unclipped (`drawsetsyms` calls `setclipping(FALSE)`) but
/// skip data points outside the world window (`is_validWPoint`).
pub fn draw_sets(canvas: &mut Canvas, gno: usize, graph: &Graph) {
    let wt = WorldTransform::new(graph);
    let v = graph.view;
    let chart_layout = chart_layout_map(graph);
    let stacked = graph.graph_type == GraphType::Chart && graph.stacked;
    let layout_of = |set: &Set| {
        chart_layout
            .get(&(set as *const Set as usize))
            .map(|(o, r)| (*o, r.as_deref()))
            .unwrap_or((0.0, None))
    };
    canvas.set_clip_view(v.xmin, v.ymin, v.xmax, v.ymax);

    for (si, set) in graph.sets.iter().enumerate() {
        if set.hidden {
            continue;
        }
        canvas.push_element(ElementId::Set { graph: gno, set: si });
        // Chart graphs offset every drawable set sideways (the bar-group
        // spacing accumulates over all sets in plotone's GRAPH_CHART loop);
        // stacked charts shift every element by the running category totals
        // (refy) instead.
        let (off, refy) = layout_of(set);
        // Grace fills from inside drawsetline, so only set types whose
        // dispatch calls it get a fill — not hilo or xyr.
        if !matches!(set.set_type, SetType::XyHiLo | SetType::XyR) {
            draw_set_fill(canvas, &wt, graph, set, refy, off);
        }
        if set.dropline {
            draw_droplines(canvas, &wt, graph, set, refy, off);
        }
        if !stacked {
            draw_set_errbars(canvas, &wt, graph, set, off, refy);
        }
        // Per-type extras and which standard elements draw, per plotone's
        // dispatch: hilo replaces line+symbols entirely; boxplot keeps the
        // connecting line; vmap and xyr keep symbols.
        let (want_line, want_syms) = match set.set_type {
            SetType::XyHiLo => {
                draw_hilo(canvas, &wt, set);
                (false, false)
            }
            SetType::BoxPlot => {
                draw_set_line(canvas, &wt, set, refy, off);
                draw_boxplot(canvas, &wt, graph, set);
                (false, false)
            }
            SetType::XyVMap => {
                draw_set_line(canvas, &wt, set, refy, off);
                draw_vmap(canvas, &wt, graph, set);
                (false, true)
            }
            SetType::XyR => {
                draw_circlexy(canvas, &wt, set);
                (false, true)
            }
            t if is_bar(t) => {
                draw_one_bar_set(canvas, &wt, graph, set, off, refy);
                (true, false)
            }
            _ => (true, true),
        };
        if want_line {
            draw_set_line(canvas, &wt, set, refy, off);
        }
        // Stacked charts draw symbols and avalues in a second pass over all
        // sets, so they sit on top of every fill (plotone GRAPH_CHART).
        if !stacked {
            canvas.clear_clip();
            if want_syms {
                draw_set_symbols(canvas, &wt, graph, set, refy, off);
            }
            if set.avalue.active {
                draw_avalues(canvas, &wt, set, off, refy);
            }
            canvas.set_clip_view(v.xmin, v.ymin, v.xmax, v.ymax);
        }
        canvas.pop_element();
    }

    if stacked {
        for (si, set) in graph.sets.iter().enumerate() {
            if set.hidden {
                continue;
            }
            canvas.push_element(ElementId::Set { graph: gno, set: si });
            let (off, refy) = layout_of(set);
            draw_set_errbars(canvas, &wt, graph, set, off, refy);
            canvas.clear_clip();
            if !is_bar(set.set_type) {
                draw_set_symbols(canvas, &wt, graph, set, refy, off);
            }
            if set.avalue.active {
                draw_avalues(canvas, &wt, set, off, refy);
            }
            canvas.set_clip_view(v.xmin, v.ymin, v.xmax, v.ymax);
            canvas.pop_element();
        }
    }
    canvas.clear_clip();
}

/// Per-set chart layout: the horizontal group offset of side-by-side bar
/// sets and, for stacked charts, the cumulative category totals *before*
/// each set (the `refy` Grace passes to drawsetbars/drawsetavalues).
#[allow(clippy::type_complexity)]
fn chart_layout_map(graph: &Graph) -> std::collections::HashMap<usize, (f64, Option<Vec<f64>>)> {
    let mut map = std::collections::HashMap::new();
    if graph.graph_type != GraphType::Chart {
        return map;
    }
    let all: Vec<&Set> = graph
        .sets
        .iter()
        .filter(|s| !s.hidden && !s.data.is_empty())
        .collect();
    if graph.stacked {
        let cats = all.iter().map(|s| s.data.len()).max().unwrap_or(0);
        let mut accum = vec![0.0f64; cats];
        for set in &all {
            map.insert(*set as *const Set as usize, (0.0, Some(accum.clone())));
            if let Some(ys) = set.data.y() {
                for (slot, y) in accum.iter_mut().zip(ys) {
                    *slot += y;
                }
            }
        }
    } else {
        // Every drawable set takes part in the side-by-side spacing, not
        // just the bars (plotone GRAPH_CHART offset accumulation).
        let mut offset = 0.0;
        for s in &all {
            offset -= 0.5 * 0.02 * s.symbol_size;
        }
        offset -= 0.5 * (all.len().saturating_sub(1)) as f64 * graph.bargap;
        for set in &all {
            offset += 0.5 * 0.02 * set.symbol_size;
            map.insert(*set as *const Set as usize, (offset, None));
            offset += 0.5 * 0.02 * set.symbol_size + graph.bargap;
        }
    }
    map
}

/// Draw the annotated values at each data point (plotone.cpp
/// `drawsetavalues`): the formatted X/Y/Z value or the per-point string,
/// centered above the point (JUST_CENTER|JUST_BOTTOM) at `avalue offset`.
fn draw_avalues(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    set: &Set,
    group_offset: f64,
    refy: Option<&[f64]>,
) {
    let av = &set.avalue;
    if av.avtype == 0 {
        return;
    }
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    let z = set.data.cols.get(2);
    let skip = set.symskip.max(0) as usize + 1;
    // Value labels draw at the avalue alpha (plotone.cpp
    // setalpha(avalue.alpha) in drawsetavalues).
    canvas.set_alpha(av.alpha);
    for i in (0..n).step_by(skip) {
        let wx = xs[i];
        let wy = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        if !wt.valid_wpoint(wx, wy) {
            continue;
        }
        let value = match av.avtype {
            1 => crate::draw::axes::format_value(wx, av.format, av.prec),
            2 => crate::draw::axes::format_value(wy, av.format, av.prec),
            3 => format!(
                "{}, {}",
                crate::draw::axes::format_value(wx, av.format, av.prec),
                crate::draw::axes::format_value(wy, av.format, av.prec)
            ),
            4 => match set.data.strs.get(i).and_then(|s| s.clone()) {
                Some(s) => s,
                None => continue,
            },
            5 => match z.and_then(|c| c.get(i)) {
                Some(&zv) => crate::draw::axes::format_value(zv, av.format, av.prec),
                None => continue,
            },
            _ => continue,
        };
        let text = format!("{}{}{}", av.prepend, value, av.append);
        let (vx, vy) = wt.world_to_view(wx, wy);
        canvas.draw_text(
            VPoint {
                x: vx + av.offx + group_offset,
                y: vy + av.offy,
            },
            &text,
            av.size,
            av.font,
            av.color,
            crate::render::HAlign::Center,
            crate::render::VAlign::Bottom,
            av.angle,
        );
    }
    canvas.set_alpha(255);
}

/// Error-bar column indices for dx+, dx-, dy+, dy- (`None` = absent).
type ErrCols = (Option<usize>, Option<usize>, Option<usize>, Option<usize>);

/// Error-bar column layout for a set type (drawseterrbars): indices into
/// `data.cols` for dx+, dx-, dy+, dy-.
fn errbar_columns(t: SetType) -> Option<ErrCols> {
    Some(match t {
        SetType::XyDx => (Some(2), None, None, None),
        SetType::XyDy | SetType::BarDy => (None, None, Some(2), None),
        SetType::XyDxDx => (Some(2), Some(3), None, None),
        SetType::XyDyDy | SetType::BarDyDy => (None, None, Some(2), Some(3)),
        SetType::XyDxDy => (Some(2), None, Some(3), None),
        SetType::XyDxDxDyDy => (Some(2), Some(3), Some(4), Some(5)),
        _ => return None,
    })
}

/// Draw a set's error bars (literal port of plotone.cpp `drawseterrbars` /
/// `drawerrorbar`): a riser from the point to point±delta with the riser
/// pen, and a perpendicular cap of half-length `0.01*size` with the bar pen.
fn draw_set_errbars(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    graph: &Graph,
    set: &Set,
    group_offset: f64,
    refy: Option<&[f64]>,
) {
    let eb = &set.errbar;
    if !eb.active {
        return;
    }
    let Some(cols) = errbar_columns(set.set_type) else {
        return;
    };
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    let col = |idx: Option<usize>| idx.and_then(|c| set.data.cols.get(c)).map(|c| c.as_slice());
    let (mut dxp, mut dxm, mut dyp, mut dym) =
        (col(cols.0), col(cols.1), col(cols.2), col(cols.3));
    // Placement: "opposite" swaps the sides, "both" mirrors the plus side
    // when no explicit minus column exists.
    match eb.place {
        1 => {
            std::mem::swap(&mut dxp, &mut dxm);
            std::mem::swap(&mut dyp, &mut dym);
        }
        2 if dxm.is_none() && dym.is_none() => {
            dxm = dxp;
            dym = dyp;
        }
        _ => {}
    }

    for i in 0..n {
        let wy = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        let (vx, vy) = wt.world_to_view(xs[i], wy);
        let p1 = VPoint { x: vx + group_offset, y: vy };
        let mut bar = |wx2: f64, wy2: f64| {
            let (v2x, v2y) = wt.world_to_view(wx2, wy2);
            draw_one_errbar(canvas, graph, eb, p1, VPoint { x: v2x + group_offset, y: v2y });
        };
        if let Some(d) = dxp {
            if let Some(&dv) = d.get(i) {
                bar(xs[i] + dv, wy);
            }
        }
        if let Some(d) = dxm {
            if let Some(&dv) = d.get(i) {
                bar(xs[i] - dv, wy);
            }
        }
        if let Some(d) = dyp {
            if let Some(&dv) = d.get(i) {
                bar(xs[i], wy + dv);
            }
        }
        if let Some(d) = dym {
            if let Some(&dv) = d.get(i) {
                bar(xs[i], wy - dv);
            }
        }
    }
}

/// One riser + cap (plotone.cpp `drawerrorbar`). With `arrow_clip` on and the
/// endpoint outside the viewport, the riser is cut at `cliplen` and finished
/// with an open arrowhead of length `2*size`.
fn draw_one_errbar(canvas: &mut Canvas, graph: &Graph, eb: &crate::model::ErrBar, vp1: VPoint, vp2: VPoint) {
    let (lx, ly) = (vp2.x - vp1.x, vp2.y - vp1.y);
    let vlength = (lx * lx + ly * ly).sqrt();
    if vlength == 0.0 {
        return;
    }
    let (ux, uy) = (lx / vlength, ly / vlength);
    let v = graph.view;
    let outside = vp2.x < v.xmin || vp2.x > v.xmax || vp2.y < v.ymin || vp2.y > v.ymax;

    // Error bars draw with the errbar pen alpha (plotone.cpp
    // setalpha(p->errbar.pen.alpha) around each part).
    canvas.set_alpha(eb.alpha);
    if eb.arrow_clip && outside {
        let vp2c = VPoint {
            x: vp1.x + eb.cliplen * ux,
            y: vp1.y + eb.cliplen * uy,
        };
        canvas.draw_polyline(&[vp1, vp2c], eb.color, eb.riser_linewidth, eb.riser_linestyle);
        // Open arrowhead, length 2*barsize (drawerrorbar: arrow.length).
        let big_l = 0.01 * (2.0 * eb.size);
        let vpc = VPoint { x: vp2c.x - big_l * ux, y: vp2c.y - big_l * uy };
        let vpl = VPoint { x: vpc.x + 0.5 * big_l * uy, y: vpc.y - 0.5 * big_l * ux };
        let vpr = VPoint { x: vpc.x - 0.5 * big_l * uy, y: vpc.y + 0.5 * big_l * ux };
        canvas.draw_polyline(&[vpl, vp2c, vpr], eb.color, eb.linewidth, 1);
        canvas.set_alpha(255);
        return;
    }

    canvas.draw_polyline(&[vp1, vp2], eb.color, eb.riser_linewidth, eb.riser_linestyle);
    let ilen = 0.01 * eb.size;
    let minus = VPoint { x: vp2.x - ilen * uy, y: vp2.y + ilen * ux };
    let plus = VPoint { x: vp2.x + ilen * uy, y: vp2.y - ilen * ux };
    canvas.draw_polyline(&[minus, plus], eb.color, eb.linewidth, eb.linestyle);
    canvas.set_alpha(255);
}

/// Hi/Lo/Open/Close set (plotone.cpp `drawsethilo`): a vertical line from
/// y1 (high) to y2 (low), an "open" tick to the left at y3 and a "close"
/// tick to the right at y4, all with the symbol pen.
fn draw_hilo(canvas: &mut Canvas, wt: &WorldTransform, set: &Set) {
    if set.symbol_linestyle == 0 {
        return;
    }
    let cols = &set.data.cols;
    let (Some(xs), Some(y1), Some(y2), Some(y3), Some(y4)) = (
        cols.first(),
        cols.get(1),
        cols.get(2),
        cols.get(3),
        cols.get(4),
    ) else {
        return;
    };
    let ilen = 0.02 * set.symbol_size;
    let n = xs.len().min(y1.len()).min(y2.len()).min(y3.len()).min(y4.len());
    // Hilo bars draw with the symbol pen, alpha included.
    canvas.set_alpha(set.symbol_pen.alpha);
    for i in 0..n {
        let (color, lw, ls) = (set.symbol_pen.color, set.symbol_linewidth, set.symbol_linestyle);
        let (x1, vy1) = wt.world_to_view(xs[i], y1[i]);
        let (_, vy2) = wt.world_to_view(xs[i], y2[i]);
        canvas.draw_polyline(&[VPoint { x: x1, y: vy1 }, VPoint { x: x1, y: vy2 }], color, lw, ls);
        let (_, vy3) = wt.world_to_view(xs[i], y3[i]);
        canvas.draw_polyline(
            &[VPoint { x: x1, y: vy3 }, VPoint { x: x1 - ilen, y: vy3 }],
            color,
            lw,
            ls,
        );
        let (_, vy4) = wt.world_to_view(xs[i], y4[i]);
        canvas.draw_polyline(
            &[VPoint { x: x1, y: vy4 }, VPoint { x: x1 + ilen, y: vy4 }],
            color,
            lw,
            ls,
        );
    }
    canvas.set_alpha(255);
}

/// Boxplot set (plotone.cpp `drawsetboxplot`): per point a box from the
/// lower to the upper bound (half-width 0.01*symsize) with a median line,
/// and error-bar whiskers from the box edges to the whisker values.
fn draw_boxplot(canvas: &mut Canvas, wt: &WorldTransform, graph: &Graph, set: &Set) {
    let cols = &set.data.cols;
    let (Some(xs), Some(md), Some(lb), Some(ub), Some(lw_), Some(uw)) = (
        cols.first(),
        cols.get(1),
        cols.get(2),
        cols.get(3),
        cols.get(4),
        cols.get(5),
    ) else {
        return;
    };
    let size = 0.01 * set.symbol_size;
    let n = [xs.len(), md.len(), lb.len(), ub.len(), lw_.len(), uw.len()]
        .into_iter()
        .min()
        .unwrap_or(0);
    for i in 0..n {
        let (vx, vlb) = wt.world_to_view(xs[i], lb[i]);
        let (_, vub) = wt.world_to_view(xs[i], ub[i]);
        // Whiskers from the box edges to the whisker values.
        if set.errbar.active {
            let (_, vlw) = wt.world_to_view(xs[i], lw_[i]);
            let (_, vuw) = wt.world_to_view(xs[i], uw[i]);
            draw_one_errbar(canvas, graph, &set.errbar, VPoint { x: vx, y: vlb }, VPoint { x: vx, y: vlw });
            draw_one_errbar(canvas, graph, &set.errbar, VPoint { x: vx, y: vub }, VPoint { x: vx, y: vuw });
        }
        // Box: symbol fill pen, then the symbol pen outline.
        let rect = [
            VPoint { x: vx - size, y: vlb },
            VPoint { x: vx + size, y: vlb },
            VPoint { x: vx + size, y: vub },
            VPoint { x: vx - size, y: vub },
        ];
        if set.symbol_fill.pattern != 0 {
            canvas.set_alpha(set.symbol_fill.alpha);
            canvas.fill_polygon(&rect, set.symbol_fill.color, set.symbol_fill.pattern);
            canvas.set_alpha(255);
        }
        if set.symbol_linestyle != 0 {
            canvas.set_alpha(set.symbol_pen.alpha);
            let mut closed = rect.to_vec();
            closed.push(rect[0]);
            canvas.draw_polyline(&closed, set.symbol_pen.color, set.symbol_linewidth, set.symbol_linestyle);
            // Median line across the box.
            let (_, vmd) = wt.world_to_view(xs[i], md[i]);
            canvas.draw_polyline(
                &[VPoint { x: vx - size, y: vmd }, VPoint { x: vx + size, y: vmd }],
                set.symbol_pen.color,
                set.symbol_linewidth,
                set.symbol_linestyle,
            );
            canvas.set_alpha(255);
        }
    }
}

/// XYR set (plotone.cpp `drawcirclexy`): an ellipse inscribed in the world
/// rectangle (x-r, y-r)..(x+r, y+r) — a circle when both world scales are
/// equal — filled with the set fill pen, outlined with the line pen. Points
/// outside the world window are skipped.
fn draw_circlexy(canvas: &mut Canvas, wt: &WorldTransform, set: &Set) {
    let cols = &set.data.cols;
    let (Some(xs), Some(ys), Some(rs)) = (cols.first(), cols.get(1), cols.get(2)) else {
        return;
    };
    let n = xs.len().min(ys.len()).min(rs.len());
    for i in 0..n {
        if !wt.valid_wpoint(xs[i], ys[i]) {
            continue;
        }
        let (x1, y1) = wt.world_to_view(xs[i] - rs[i], ys[i] - rs[i]);
        let (x2, y2) = wt.world_to_view(xs[i] + rs[i], ys[i] + rs[i]);
        let (p1, p2) = (VPoint { x: x1, y: y1 }, VPoint { x: x2, y: y2 });
        if set.fill_type != FillType::None && set.fill_pen.pattern != 0 {
            canvas.set_alpha(set.fill_pen.alpha);
            canvas.fill_ellipse(p1, p2, set.fill_pen.color, set.fill_pen.pattern);
        }
        if set.linestyle != 0 {
            canvas.set_alpha(set.line_pen.alpha);
            canvas.stroke_ellipse(p1, p2, set.line_pen.color, set.linewidth, set.linestyle);
        }
        canvas.set_alpha(255);
    }
}

/// Vector-map set (plotone.cpp `drawsetvmap`): an arrow from each point,
/// the vector (vx, vy)/znorm applied in *view* units; riser and head use
/// the error-bar pens, the open head is 2*errbar.size long.
fn draw_vmap(canvas: &mut Canvas, wt: &WorldTransform, graph: &Graph, set: &Set) {
    if graph.znorm == 0.0 {
        return;
    }
    let cols = &set.data.cols;
    let (Some(xs), Some(ys), Some(vxs), Some(vys)) =
        (cols.first(), cols.get(1), cols.get(2), cols.get(3))
    else {
        return;
    };
    let eb = &set.errbar;
    let n = xs.len().min(ys.len()).min(vxs.len()).min(vys.len());
    // Arrows draw with the error-bar pen, alpha included.
    canvas.set_alpha(eb.alpha);
    for i in 0..n {
        if !wt.valid_wpoint(xs[i], ys[i]) {
            continue;
        }
        let (vx, vy) = wt.world_to_view(xs[i], ys[i]);
        let p1 = VPoint { x: vx, y: vy };
        let p2 = VPoint {
            x: vx + vxs[i] / graph.znorm,
            y: vy + vys[i] / graph.znorm,
        };
        canvas.draw_polyline(&[p1, p2], eb.color, eb.riser_linewidth, eb.riser_linestyle);
        // Open arrowhead at p2 (draw_arrowhead, type 0, length 2*barsize).
        let (lx, ly) = (p2.x - p1.x, p2.y - p1.y);
        let vlen = (lx * lx + ly * ly).sqrt();
        if vlen == 0.0 {
            continue;
        }
        let (ux, uy) = (lx / vlen, ly / vlen);
        let big_l = 0.01 * (2.0 * eb.size);
        let vpc = VPoint { x: p2.x - big_l * ux, y: p2.y - big_l * uy };
        let vpl = VPoint { x: vpc.x + 0.5 * big_l * uy, y: vpc.y - 0.5 * big_l * ux };
        let vpr = VPoint { x: vpc.x - 0.5 * big_l * uy, y: vpc.y + 0.5 * big_l * ux };
        canvas.draw_polyline(&[vpl, p2, vpr], eb.color, eb.linewidth, 1);
    }
    canvas.set_alpha(255);
}

/// Draw vertical droplines from each point down to the set's baseline; in
/// stacked charts the line runs from the previous totals to the stacked
/// point, and chart group offsets shift it sideways (drawsetline).
fn draw_droplines(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    graph: &Graph,
    set: &Set,
    refy: Option<&[f64]>,
    off: f64,
) {
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    let ybase = baseline_value(graph, set, ys, n);
    // Droplines draw with the line pen (drawsetline), alpha included.
    canvas.set_alpha(set.line_pen.alpha);
    for i in 0..n {
        let r = refy.and_then(|r| r.get(i)).copied();
        let (base, top) = match r {
            Some(b) => (b, b + ys[i]),
            None => (ybase, ys[i]),
        };
        let (vx, vyb) = wt.world_to_view(xs[i], base);
        let (_, vyt) = wt.world_to_view(xs[i], top);
        canvas.draw_polyline(
            &[VPoint { x: vx + off, y: vyb }, VPoint { x: vx + off, y: vyt }],
            set.line_pen.color,
            set.linewidth,
            if set.linestyle == 0 { 1 } else { set.linestyle },
        );
    }
    canvas.set_alpha(255);
}

/// Draw the bars of a single set: each point becomes a rectangle of
/// half-width 0.01*symsize from the baseline (or the stacked totals) to the
/// value, shifted by the chart group offset (plotone.cpp drawsetbars).
fn draw_one_bar_set(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    graph: &Graph,
    set: &Set,
    offset: f64,
    refy: Option<&[f64]>,
) {
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    let ybase = baseline_value(graph, set, ys, n);
    let bw = 0.01 * set.symbol_size; // bar half-width in view units
    let do_fill = set.symbol_fill.pattern != 0;
    let do_outline = set.symbol_linestyle != 0;

    for i in 0..n {
        let (base_y, top_y) = match refy.and_then(|r| r.get(i)).copied() {
            Some(b) => (b, b + ys[i]),
            None => (ybase, ys[i]),
        };
        let (bx, by) = wt.world_to_view(xs[i], base_y);
        let (tx, ty) = wt.world_to_view(xs[i], top_y);
        let x1 = bx + offset - bw;
        let x2 = tx + offset + bw;
        let rect = [
            VPoint { x: x1, y: by },
            VPoint { x: x2, y: by },
            VPoint { x: x2, y: ty },
            VPoint { x: x1, y: ty },
        ];
        if do_fill {
            // drawsetbars: setpen(p->symfillpen), alpha included.
            canvas.set_alpha(set.symbol_fill.alpha);
            canvas.fill_polygon(&rect, set.symbol_fill.color, set.symbol_fill.pattern);
        }
        if do_outline {
            canvas.set_alpha(set.symbol_pen.alpha);
            let mut closed = rect.to_vec();
            closed.push(rect[0]);
            canvas.draw_polyline(&closed, set.symbol_pen.color, set.symbol_linewidth, set.symbol_linestyle);
        }
        canvas.set_alpha(255);
    }
}

/// Fill the area of a set: the closed data polygon, or between the curve and a
/// baseline. Drawn with the set's fill pen.
fn draw_set_fill(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    graph: &Graph,
    set: &Set,
    refy: Option<&[f64]>,
    off: f64,
) {
    // Grace's drawsetfill only fills when a line type defines the path; bar
    // sets (and any set with line type "none") get no polygon fill.
    if set.fill_type == FillType::None
        || set.fill_pen.pattern == 0
        || set.line_type == LineType::None
    {
        return;
    }
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    if n < 2 {
        return;
    }
    // The fill draws with the set fill pen, alpha included (drawsetfill:
    // setpen(p->setfillpen); QtGrace pens carry an alpha channel).
    canvas.set_alpha(set.fill_pen.alpha);
    let mut pts: Vec<VPoint> = Vec::with_capacity(2 * n + 2);
    for (i, &x) in xs[..n].iter().enumerate() {
        let y = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        let (vx, vy) = wt.world_to_view(x, y);
        pts.push(VPoint { x: vx + off, y: vy });
    }
    // Stacked-chart baseline fill closes back along the previous sets' totals
    // (drawsetfill: vps[len..2len] = reversed refy), not along a flat line.
    if let Some(r) = refy {
        if set.fill_type == FillType::Baseline {
            for i in (0..n).rev() {
                let (vx, vy) = wt.world_to_view(xs[i], r.get(i).copied().unwrap_or(0.0));
                pts.push(VPoint { x: vx + off, y: vy });
            }
            canvas.fill_polygon_rule(&pts, set.fill_pen.color, set.fill_pen.pattern, set.fill_rule);
            canvas.set_alpha(255);
            return;
        }
    }
    if set.fill_type == FillType::Baseline {
        // Close the polygon along the baseline between the set's x extent
        // clamped to the world window: Grace `drawsetfill` (plotone.cpp)
        // appends (MIN2(xmax, w.xg2), ybase) then (MAX2(xmin, w.xg1), ybase).
        let ybase = baseline_value(graph, set, ys, n);
        let xmin = xs[..n].iter().copied().fold(f64::INFINITY, f64::min);
        let xmax = xs[..n].iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let w = &graph.world;
        let (vxr, vyb) = wt.world_to_view(xmax.min(w.xmax), ybase);
        let (vxl, vyb2) = wt.world_to_view(xmin.max(w.xmin), ybase);
        pts.push(VPoint { x: vxr + off, y: vyb });
        pts.push(VPoint { x: vxl + off, y: vyb2 });
    }
    canvas.fill_polygon_rule(&pts, set.fill_pen.color, set.fill_pen.pattern, set.fill_rule);
    canvas.set_alpha(255);
}

/// Baseline Y value for baseline fills (Grace `setybase`).
fn baseline_value(graph: &Graph, set: &Set, ys: &[f64], n: usize) -> f64 {
    match set.baseline_type {
        1 => ys[..n].iter().copied().fold(f64::INFINITY, f64::min), // SMIN
        2 => ys[..n].iter().copied().fold(f64::NEG_INFINITY, f64::max), // SMAX
        3 => graph.world.ymin,                                      // GMIN
        4 => graph.world.ymax,                                      // GMAX
        5 => ys[..n].iter().sum::<f64>() / n as f64,                // SAVG
        _ => 0.0,                                                   // TYPE_0
    }
}

/// Draw the polyline connecting a set's points (if its line type calls for one).
fn draw_set_line(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    set: &Set,
    refy: Option<&[f64]>,
    off: f64,
) {
    if set.line_type == LineType::None || set.linestyle == 0 {
        return;
    }
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    if n < 2 {
        return;
    }
    // Line pen alpha (drawsetline sets the line pen; QtGrace ALPHA_CHANNELS).
    canvas.set_alpha(set.line_pen.alpha);

    // Convert points to view coordinates. Out-of-domain values (e.g. y <= 0
    // on a log axis) map to view 0 and the viewport clip trims the segment,
    // exactly as in Grace (xy_yconv_general + clip_line) — lines are never
    // broken at domain gaps.
    let at = |i: usize| {
        // Stacked charts draw the line at the accumulated height
        // (drawsetline: wptmp.y += refy[i]); chart group offsets shift x.
        let y = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        let (vx, vy) = wt.world_to_view(xs[i], y);
        VPoint { x: vx + off, y: vy }
    };

    // Segment line types draw disconnected runs of 2 / 3 points
    // (drawsetline LINE_TYPE_SEGMENT2/3).
    let group = match set.line_type {
        LineType::Segment2 => 2,
        LineType::Segment3 => 3,
        _ => 0,
    };
    if group > 0 {
        let mut i = 0;
        while i < n {
            let seg: Vec<VPoint> = (i..n.min(i + group)).map(&at).collect();
            if seg.len() >= 2 {
                canvas.draw_polyline(&seg, set.line_pen.color, set.linewidth, set.linestyle);
            }
            i += group;
        }
        canvas.set_alpha(255);
        return;
    }

    let mut segment: Vec<VPoint> = Vec::with_capacity(2 * n);
    for i in 0..n {
        let p = at(i);
        // Insert the stair step vertex between consecutive points.
        if let Some(&prev) = segment.last() {
            match set.line_type {
                LineType::LeftStair => segment.push(VPoint { x: prev.x, y: p.y }),
                LineType::RightStair => segment.push(VPoint { x: p.x, y: prev.y }),
                _ => {}
            }
        }
        segment.push(p);
    }
    if segment.len() >= 2 {
        canvas.draw_polyline(&segment, set.line_pen.color, set.linewidth, set.linestyle);
    }
    canvas.set_alpha(255);
}

/// Draw the plot symbol at each data point of a set.
fn draw_set_symbols(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    graph: &Graph,
    set: &Set,
    refy: Option<&[f64]>,
    off: f64,
) {
    if set.symbol == SymbolType::None {
        return;
    }
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    // For xysize sets the third column scales the symbol size by 1/znorm.
    let zsize = if set.set_type == SetType::XySize {
        set.data.cols.get(2)
    } else {
        None
    };
    // For xycolor sets the third column is a per-point fill color index.
    let zcolor = if set.set_type == SetType::XyColor {
        set.data.cols.get(2)
    } else {
        None
    };
    let do_fill = set.symbol_fill.pattern != 0;
    let do_outline = set.symbol_linestyle != 0;

    // Pathologically dense uniform symbol clouds: two identical symbols
    // closer than half their radius are visually idempotent, so above the
    // threshold skip symbols whose center lands in an already-stamped cell
    // of that size (≥ half a pixel). Per-point size/color sets are exempt
    // (their symbols differ), as are character symbols.
    let dense_dedup = n > DENSE_SYMBOL_LIMIT
        && zsize.is_none()
        && zcolor.is_none()
        && set.symbol != SymbolType::Char;
    let side = canvas.page().side;
    let cell_view = (0.01 * set.symbol_size / 2.0).max(0.5 / side);
    let mut stamped: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();

    let skip = set.symskip.max(0) as usize + 1;
    for i in (0..n).step_by(skip) {
        let y = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        // Grace skips symbols whose data point lies outside the world window
        // (`drawsetsyms` -> `is_validWPoint`); symbols are not clipped, so one
        // sitting on the frame edge may overhang it, exactly as in Grace.
        if !wt.valid_wpoint(xs[i], y) {
            continue;
        }
        let (vx, vy) = wt.world_to_view(xs[i], y);
        let vx = vx + off;
        // Symbol radius in view units (Grace: 0.01 * symsize).
        let r = match zsize {
            Some(z) if graph.znorm != 0.0 => 0.01 * (z.get(i).copied().unwrap_or(0.0) / graph.znorm),
            _ => 0.01 * set.symbol_size,
        };
        if r <= 0.0 {
            continue;
        }
        let fill_color = match zcolor {
            // Grace rounds the per-point color index (drawsetsyms:
            // `(int) rint(c[i])`), it does not truncate.
            Some(z) => z.get(i).copied().unwrap_or(0.0).round() as i32,
            None => set.symbol_fill.color,
        };
        if dense_dedup {
            let cell = (
                (vx / cell_view).round() as i32,
                (vy / cell_view).round() as i32,
            );
            if !stamped.insert(cell) {
                continue;
            }
        }
        let c = VPoint { x: vx, y: vy };
        draw_one_symbol(canvas, set, c, r, do_fill, do_outline, fill_color);
    }
}

/// Symbol count above which dense uniform clouds are deduplicated by
/// half-radius cells.
const DENSE_SYMBOL_LIMIT: usize = 4096;

/// Draw a set's symbol centered at a view point (used by the legend swatch).
pub fn draw_symbol_at(canvas: &mut Canvas, set: &Set, c: VPoint) {
    if set.symbol == SymbolType::None {
        return;
    }
    let r = 0.01 * set.symbol_size;
    draw_one_symbol(
        canvas,
        set,
        c,
        r,
        set.symbol_fill.pattern != 0,
        set.symbol_linestyle != 0,
        set.symbol_fill.color,
    );
}

/// Draw a single symbol centered at view point `c` with view radius `r`.
/// `fc` is the fill color index (overridable per point for xycolor sets).
fn draw_one_symbol(canvas: &mut Canvas, set: &Set, c: VPoint, r: f64, fill: bool, outline: bool, fc: i32) {
    let lw = set.symbol_linewidth;
    let ls = set.symbol_linestyle;
    let oc = set.symbol_pen.color;

    // SYM_CHAR: write the configured character centered on the point at the
    // symbol size (plotone.cpp drawxysym: setcharsize(size); WriteString(vp,
    // 0, JUST_CENTER|JUST_MIDDLE, buf)). `r` is 0.01*symsize, so recover the
    // Grace char size.
    if set.symbol == SymbolType::Char {
        // Char symbols draw with the symbol pen, alpha included (drawxysym:
        // setalpha(sympen.alpha) before WriteString).
        canvas.set_alpha(set.symbol_pen.alpha);
        let s = char::from(set.symbol_char).to_string();
        canvas.draw_text(
            c,
            &s,
            r / 0.01,
            set.symbol_char_font,
            oc,
            crate::render::HAlign::Center,
            crate::render::VAlign::Middle,
            0.0,
        );
        canvas.set_alpha(255);
        return;
    }

    // Build the polygon vertices (in view units) for polygonal symbols.
    let poly: Option<Vec<VPoint>> = match set.symbol {
        SymbolType::Square => {
            let s = r * 0.85;
            Some(rect(c, s, s))
        }
        SymbolType::Diamond => Some(vec![
            VPoint { x: c.x, y: c.y + r },
            VPoint { x: c.x - r, y: c.y },
            VPoint { x: c.x, y: c.y - r },
            VPoint { x: c.x + r, y: c.y },
        ]),
        SymbolType::TriangleUp => Some(vec![
            VPoint { x: c.x, y: c.y + 2.0 * SQRT1_3 * r },
            VPoint { x: c.x - r, y: c.y - SQRT1_3 * r },
            VPoint { x: c.x + r, y: c.y - SQRT1_3 * r },
        ]),
        SymbolType::TriangleLeft => Some(vec![
            VPoint { x: c.x - 2.0 * SQRT1_3 * r, y: c.y },
            VPoint { x: c.x + SQRT1_3 * r, y: c.y - r },
            VPoint { x: c.x + SQRT1_3 * r, y: c.y + r },
        ]),
        SymbolType::TriangleDown => Some(vec![
            VPoint { x: c.x - r, y: c.y + SQRT1_3 * r },
            VPoint { x: c.x, y: c.y - 2.0 * SQRT1_3 * r },
            VPoint { x: c.x + r, y: c.y + SQRT1_3 * r },
        ]),
        SymbolType::TriangleRight => Some(vec![
            VPoint { x: c.x - SQRT1_3 * r, y: c.y + r },
            VPoint { x: c.x - SQRT1_3 * r, y: c.y - r },
            VPoint { x: c.x + 2.0 * SQRT1_3 * r, y: c.y },
        ]),
        _ => None,
    };

    // Symbol interiors fill with the symbol fill pen's alpha, outlines
    // stroke with the symbol pen's (drawxysym: setpen(fillpen) then
    // setalpha(sympen.alpha); the xycolor per-point override replaces the
    // color only, drawsetline keeps fillpen.alpha = symfillpen.alpha).
    if let Some(pts) = poly {
        if fill {
            canvas.set_alpha(set.symbol_fill.alpha);
            canvas.fill_polygon(&pts, fc, set.symbol_fill.pattern);
        }
        if outline {
            canvas.set_alpha(set.symbol_pen.alpha);
            let mut closed = pts.clone();
            closed.push(pts[0]);
            canvas.draw_polyline(&closed, oc, lw, ls);
        }
        canvas.set_alpha(255);
        return;
    }

    canvas.set_alpha(set.symbol_pen.alpha);
    match set.symbol {
        SymbolType::Circle => {
            if fill {
                canvas.set_alpha(set.symbol_fill.alpha);
                canvas.fill_circle(c, r, fc, set.symbol_fill.pattern);
            }
            if outline {
                canvas.set_alpha(set.symbol_pen.alpha);
                canvas.stroke_circle(c, r, oc, lw, ls);
            }
        }
        SymbolType::Plus => {
            canvas.draw_polyline(&[VPoint { x: c.x - r, y: c.y }, VPoint { x: c.x + r, y: c.y }], oc, lw, ls);
            canvas.draw_polyline(&[VPoint { x: c.x, y: c.y - r }, VPoint { x: c.x, y: c.y + r }], oc, lw, ls);
        }
        SymbolType::Cross => {
            let d = SQRT1_2 * r;
            canvas.draw_polyline(&[VPoint { x: c.x - d, y: c.y - d }, VPoint { x: c.x + d, y: c.y + d }], oc, lw, ls);
            canvas.draw_polyline(&[VPoint { x: c.x - d, y: c.y + d }, VPoint { x: c.x + d, y: c.y - d }], oc, lw, ls);
        }
        SymbolType::Star => {
            // "Splat": a plus and a cross combined.
            canvas.draw_polyline(&[VPoint { x: c.x - r, y: c.y }, VPoint { x: c.x + r, y: c.y }], oc, lw, ls);
            canvas.draw_polyline(&[VPoint { x: c.x, y: c.y - r }, VPoint { x: c.x, y: c.y + r }], oc, lw, ls);
            let d = SQRT1_2 * r;
            canvas.draw_polyline(&[VPoint { x: c.x - d, y: c.y - d }, VPoint { x: c.x + d, y: c.y + d }], oc, lw, ls);
            canvas.draw_polyline(&[VPoint { x: c.x - d, y: c.y + d }, VPoint { x: c.x + d, y: c.y - d }], oc, lw, ls);
        }
        // Char symbols are deferred (rare); None already returned above.
        _ => {}
    }
    canvas.set_alpha(255);
}

/// Axis-aligned rectangle (4 corners) centered at `c` with half-extents.
fn rect(c: VPoint, hx: f64, hy: f64) -> Vec<VPoint> {
    vec![
        VPoint { x: c.x - hx, y: c.y - hy },
        VPoint { x: c.x - hx, y: c.y + hy },
        VPoint { x: c.x + hx, y: c.y + hy },
        VPoint { x: c.x + hx, y: c.y - hy },
    ]
}
