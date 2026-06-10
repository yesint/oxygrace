//! Dataset drawing. Milestone 1 draws the connecting line of XY-like sets;
//! symbols, fills and error bars are added in later milestones.

use crate::model::{FillType, GraphType, Graph, LineType, Set, SetType, SymbolType};
use crate::render::{Canvas, VPoint, WorldTransform};

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
pub fn draw_sets(canvas: &mut Canvas, graph: &Graph) {
    let wt = WorldTransform::new(graph);
    let v = graph.view;
    let chart_layout = chart_layout_map(graph);
    canvas.set_clip_view(v.xmin, v.ymin, v.xmax, v.ymax);
    // Bars are drawn first (as a grouped pass) so lines/symbols sit on top.
    draw_bars(canvas, &wt, graph);

    for set in &graph.sets {
        if set.hidden {
            continue;
        }
        // Fill is drawn under the line, then symbols on top (Grace order).
        // Bar-type sets are rendered entirely by draw_bars: Grace never draws
        // a connecting line or symbols for them, even if those properties are
        // set in the file. Stacked charts shift every element by the running
        // category totals (refy), exactly as plotone passes refy through.
        let (off, refy) = chart_layout
            .get(&(set as *const Set as usize))
            .map(|(o, r)| (*o, r.as_deref()))
            .unwrap_or((0.0, None));
        draw_set_fill(canvas, &wt, graph, set, refy);
        if set.dropline {
            draw_droplines(canvas, &wt, graph, set);
        }
        draw_set_errbars(canvas, &wt, graph, set, off, refy);
        if !is_bar(set.set_type) {
            draw_set_line(canvas, &wt, set, refy);
        }
        canvas.clear_clip();
        if !is_bar(set.set_type) {
            draw_set_symbols(canvas, &wt, graph, set, refy);
        }
        if set.avalue.active {
            draw_avalues(canvas, &wt, graph, set, off, refy);
        }
        canvas.set_clip_view(v.xmin, v.ymin, v.xmax, v.ymax);
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
    let bars: Vec<&Set> = graph
        .sets
        .iter()
        .filter(|s| !s.hidden && is_bar(s.set_type) && !s.data.is_empty())
        .collect();
    if graph.stacked {
        let all: Vec<&Set> = graph
            .sets
            .iter()
            .filter(|s| !s.hidden && !s.data.is_empty())
            .collect();
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
        let mut offset = 0.0;
        for s in &bars {
            offset -= 0.5 * 0.02 * s.symbol_size;
        }
        offset -= 0.5 * (bars.len().saturating_sub(1)) as f64 * graph.bargap;
        for set in &bars {
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
    graph: &Graph,
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
    let w = &graph.world;
    let (wx0, wx1) = (w.xmin.min(w.xmax), w.xmin.max(w.xmax));
    let (wy0, wy1) = (w.ymin.min(w.ymax), w.ymin.max(w.ymax));

    for i in 0..n {
        let wx = xs[i];
        let wy = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        if wx < wx0 || wx > wx1 || wy < wy0 || wy > wy1 {
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
        2 => {
            if dxm.is_none() && dym.is_none() {
                dxm = dxp;
                dym = dyp;
            }
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
        return;
    }

    canvas.draw_polyline(&[vp1, vp2], eb.color, eb.riser_linewidth, eb.riser_linestyle);
    let ilen = 0.01 * eb.size;
    let minus = VPoint { x: vp2.x - ilen * uy, y: vp2.y + ilen * ux };
    let plus = VPoint { x: vp2.x + ilen * uy, y: vp2.y - ilen * ux };
    canvas.draw_polyline(&[minus, plus], eb.color, eb.linewidth, eb.linestyle);
}

/// Draw vertical droplines from each point down to the set's baseline.
fn draw_droplines(canvas: &mut Canvas, wt: &WorldTransform, graph: &Graph, set: &Set) {
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    let ybase = baseline_value(graph, set, ys, n);
    for i in 0..n {
        let (vx, vyb) = wt.world_to_view(xs[i], ybase);
        let (_, vyt) = wt.world_to_view(xs[i], ys[i]);
        canvas.draw_polyline(
            &[VPoint { x: vx, y: vyb }, VPoint { x: vx, y: vyt }],
            set.line_pen.color,
            set.linewidth,
            if set.linestyle == 0 { 1 } else { set.linestyle },
        );
    }
}

/// Draw all bar sets of a graph, grouping side-by-side in chart graphs.
fn draw_bars(canvas: &mut Canvas, wt: &WorldTransform, graph: &Graph) {
    let bars: Vec<&Set> = graph
        .sets
        .iter()
        .filter(|s| !s.hidden && is_bar(s.set_type) && !s.data.is_empty())
        .collect();
    if bars.is_empty() {
        return;
    }
    let is_chart = graph.graph_type == GraphType::Chart;
    let stacked = is_chart && graph.stacked;

    if stacked {
        // Cumulative baseline per category, growing as sets are stacked.
        let cats = bars.iter().map(|s| s.data.len()).max().unwrap_or(0);
        let mut accum = vec![0.0f64; cats];
        for set in &bars {
            draw_one_bar_set(canvas, wt, set, 0.0, BarBase::Stack(&mut accum));
        }
        return;
    }

    // Side-by-side grouping: offset so the cluster is centered on each category.
    let mut offset = 0.0;
    if is_chart {
        for s in &bars {
            offset -= 0.5 * 0.02 * s.symbol_size;
        }
        offset -= 0.5 * (bars.len().saturating_sub(1)) as f64 * graph.bargap;
    }
    for set in &bars {
        if is_chart {
            offset += 0.5 * 0.02 * set.symbol_size;
        }
        let n = set.data.len();
        let ybase = baseline_value(graph, set, set.data.y().unwrap_or(&[]), n);
        draw_one_bar_set(canvas, wt, set, offset, BarBase::Fixed(ybase));
        if is_chart {
            offset += 0.5 * 0.02 * set.symbol_size + graph.bargap;
        }
    }
}

/// How a bar set's lower edge is determined.
enum BarBase<'a> {
    /// Fixed baseline Y for all bars.
    Fixed(f64),
    /// Per-category cumulative totals; advanced as bars stack.
    Stack(&'a mut Vec<f64>),
}

/// Draw the bars of a single set with a horizontal view `offset`.
fn draw_one_bar_set(canvas: &mut Canvas, wt: &WorldTransform, set: &Set, offset: f64, mut base: BarBase) {
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    let bw = 0.01 * set.symbol_size; // bar half-width in view units
    let do_fill = set.symbol_fill.pattern != 0;
    let do_outline = set.symbol_linestyle != 0;

    for i in 0..n {
        let (base_y, top_y) = match base {
            BarBase::Fixed(b) => (b, ys[i]),
            BarBase::Stack(ref mut acc) => {
                let b = acc.get(i).copied().unwrap_or(0.0);
                if let Some(slot) = acc.get_mut(i) {
                    *slot = b + ys[i];
                }
                (b, b + ys[i])
            }
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
            canvas.fill_polygon(&rect, set.symbol_fill.color, set.symbol_fill.pattern);
        }
        if do_outline {
            let mut closed = rect.to_vec();
            closed.push(rect[0]);
            canvas.draw_polyline(&closed, set.symbol_pen.color, set.symbol_linewidth, set.symbol_linestyle);
        }
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
    let mut pts: Vec<VPoint> = Vec::with_capacity(2 * n + 2);
    for (i, &x) in xs[..n].iter().enumerate() {
        let y = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        let (vx, vy) = wt.world_to_view(x, y);
        pts.push(VPoint { x: vx, y: vy });
    }
    // Stacked-chart baseline fill closes back along the previous sets' totals
    // (drawsetfill: vps[len..2len] = reversed refy), not along a flat line.
    if let Some(r) = refy {
        if set.fill_type == FillType::Baseline {
            for i in (0..n).rev() {
                let (vx, vy) = wt.world_to_view(xs[i], r.get(i).copied().unwrap_or(0.0));
                pts.push(VPoint { x: vx, y: vy });
            }
            canvas.fill_polygon_rule(&pts, set.fill_pen.color, set.fill_pen.pattern, set.fill_rule);
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
        pts.push(VPoint { x: vxr, y: vyb });
        pts.push(VPoint { x: vxl, y: vyb2 });
    }
    canvas.fill_polygon_rule(&pts, set.fill_pen.color, set.fill_pen.pattern, set.fill_rule);
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

    // Convert points to view coordinates. Out-of-domain values (e.g. y <= 0
    // on a log axis) map to view 0 and the viewport clip trims the segment,
    // exactly as in Grace (xy_yconv_general + clip_line) — lines are never
    // broken at domain gaps.
    let mut segment: Vec<VPoint> = Vec::with_capacity(2 * n);
    for i in 0..n {
        // Stacked charts draw the line at the accumulated height
        // (drawsetline: wptmp.y += refy[i]).
        let y = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        let (vx, vy) = wt.world_to_view(xs[i], y);
        let p = VPoint { x: vx, y: vy };
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
}

/// Draw the plot symbol at each data point of a set.
fn draw_set_symbols(
    canvas: &mut Canvas,
    wt: &WorldTransform,
    graph: &Graph,
    set: &Set,
    refy: Option<&[f64]>,
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

    // World window for the point-inside test (Grace `is_validWPoint`).
    let w = &graph.world;
    let (wx0, wx1) = (w.xmin.min(w.xmax), w.xmin.max(w.xmax));
    let (wy0, wy1) = (w.ymin.min(w.ymax), w.ymin.max(w.ymax));

    for i in 0..n {
        let y = ys[i] + refy.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        // Grace skips symbols whose data point lies outside the world window
        // (`drawsetsyms` -> `is_validWPoint`); symbols are not clipped, so one
        // sitting on the frame edge may overhang it, exactly as in Grace.
        if xs[i] < wx0 || xs[i] > wx1 || y < wy0 || y > wy1 {
            continue;
        }
        let (vx, vy) = wt.world_to_view(xs[i], y);
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
        let c = VPoint { x: vx, y: vy };
        draw_one_symbol(canvas, set, c, r, do_fill, do_outline, fill_color);
    }
}

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

    if let Some(pts) = poly {
        if fill {
            canvas.fill_polygon(&pts, fc, set.symbol_fill.pattern);
        }
        if outline {
            let mut closed = pts.clone();
            closed.push(pts[0]);
            canvas.draw_polyline(&closed, oc, lw, ls);
        }
        return;
    }

    match set.symbol {
        SymbolType::Circle => {
            if fill {
                canvas.fill_circle(c, r, fc, set.symbol_fill.pattern);
            }
            if outline {
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
