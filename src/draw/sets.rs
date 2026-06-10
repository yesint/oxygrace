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
        // set in the file.
        draw_set_fill(canvas, &wt, graph, set);
        if set.dropline {
            draw_droplines(canvas, &wt, graph, set);
        }
        if !is_bar(set.set_type) {
            draw_set_line(canvas, &wt, set);
            canvas.clear_clip();
            draw_set_symbols(canvas, &wt, graph, set);
            canvas.set_clip_view(v.xmin, v.ymin, v.xmax, v.ymax);
        }
    }
    canvas.clear_clip();
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
fn draw_set_fill(canvas: &mut Canvas, wt: &WorldTransform, graph: &Graph, set: &Set) {
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
    let mut pts: Vec<VPoint> = Vec::with_capacity(n + 2);
    for i in 0..n {
        let (vx, vy) = wt.world_to_view(xs[i], ys[i]);
        pts.push(VPoint { x: vx, y: vy });
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
fn draw_set_line(canvas: &mut Canvas, wt: &WorldTransform, set: &Set) {
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
        let (vx, vy) = wt.world_to_view(xs[i], ys[i]);
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
fn draw_set_symbols(canvas: &mut Canvas, wt: &WorldTransform, graph: &Graph, set: &Set) {
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
        // Grace skips symbols whose data point lies outside the world window
        // (`drawsetsyms` -> `is_validWPoint`); symbols are not clipped, so one
        // sitting on the frame edge may overhang it, exactly as in Grace.
        if xs[i] < wx0 || xs[i] > wx1 || ys[i] < wy0 || ys[i] > wy1 {
            continue;
        }
        let (vx, vy) = wt.world_to_view(xs[i], ys[i]);
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
