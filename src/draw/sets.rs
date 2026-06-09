//! Dataset drawing. Milestone 1 draws the connecting line of XY-like sets;
//! symbols, fills and error bars are added in later milestones.

use crate::model::{FillType, Graph, LineType, Set, SymbolType};
use crate::render::{Canvas, VPoint, WorldTransform};

/// 1/sqrt(3), used for equilateral-triangle symbol vertices (matches Grace).
const SQRT1_3: f64 = 0.577_350_269_189_625_8;
/// 1/sqrt(2), used for the diagonal "x" symbol.
const SQRT1_2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Draw every visible set of a graph: connecting line, then symbols.
pub fn draw_sets(canvas: &mut Canvas, graph: &Graph) {
    let wt = WorldTransform::new(graph);
    for set in &graph.sets {
        if set.hidden {
            continue;
        }
        // Fill is drawn under the line, then symbols on top (Grace order).
        draw_set_fill(canvas, &wt, graph, set);
        draw_set_line(canvas, &wt, set);
        draw_set_symbols(canvas, &wt, set);
    }
}

/// Fill the area of a set: the closed data polygon, or between the curve and a
/// baseline. Drawn with the set's fill pen.
fn draw_set_fill(canvas: &mut Canvas, wt: &WorldTransform, graph: &Graph, set: &Set) {
    if set.fill_type == FillType::None || set.fill_pen.pattern == 0 {
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
        if let Some((vx, vy)) = wt.world_to_view(xs[i], ys[i]) {
            pts.push(VPoint { x: vx, y: vy });
        }
    }
    if pts.len() < 2 {
        return;
    }
    if set.fill_type == FillType::Baseline {
        // Close the polygon back along the baseline y between the x extents.
        let ybase = baseline_value(graph, set, ys, n);
        let (x_first, x_last) = (xs[0], xs[n - 1]);
        if let (Some((vxl, vyb)), Some((vxr, _))) =
            (wt.world_to_view(x_last, ybase), wt.world_to_view(x_first, ybase))
        {
            pts.push(VPoint { x: vxl, y: vyb });
            let vyb2 = wt.y_to_view(ybase).unwrap_or(vyb);
            pts.push(VPoint { x: vxr, y: vyb2 });
        }
    }
    canvas.fill_polygon(&pts, set.fill_pen.color);
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

    // Convert points to view coordinates, breaking the line at domain gaps
    // (e.g. non-positive values on a log axis).
    let mut segment: Vec<VPoint> = Vec::with_capacity(n);
    let flush = |seg: &mut Vec<VPoint>, canvas: &mut Canvas| {
        if seg.len() >= 2 {
            canvas.draw_polyline(seg, set.line_pen.color, set.linewidth, set.linestyle);
        }
        seg.clear();
    };

    for i in 0..n {
        match wt.world_to_view(xs[i], ys[i]) {
            Some((vx, vy)) => segment.push(VPoint { x: vx, y: vy }),
            None => flush(&mut segment, canvas),
        }
    }
    flush(&mut segment, canvas);
}

/// Draw the plot symbol at each data point of a set.
fn draw_set_symbols(canvas: &mut Canvas, wt: &WorldTransform, set: &Set) {
    if set.symbol == SymbolType::None {
        return;
    }
    let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) else {
        return;
    };
    let n = xs.len().min(ys.len());
    // Symbol radius in view units (Grace: 0.01 * symsize).
    let r = 0.01 * set.symbol_size;
    if r <= 0.0 {
        return;
    }
    let do_fill = set.symbol_fill.pattern != 0;
    let do_outline = set.symbol_linestyle != 0;

    for i in 0..n {
        let Some((vx, vy)) = wt.world_to_view(xs[i], ys[i]) else {
            continue;
        };
        let c = VPoint { x: vx, y: vy };
        draw_one_symbol(canvas, set, c, r, do_fill, do_outline);
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
    );
}

/// Draw a single symbol centered at view point `c` with view radius `r`.
fn draw_one_symbol(canvas: &mut Canvas, set: &Set, c: VPoint, r: f64, fill: bool, outline: bool) {
    let lw = set.symbol_linewidth;
    let ls = set.symbol_linestyle;
    let oc = set.symbol_pen.color;
    let fc = set.symbol_fill.color;

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
            canvas.fill_polygon(&pts, fc);
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
                canvas.fill_circle(c, r, fc);
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
