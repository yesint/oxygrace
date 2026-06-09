//! Dataset drawing. Milestone 1 draws the connecting line of XY-like sets;
//! symbols, fills and error bars are added in later milestones.

use crate::model::{Graph, LineType, Set};
use crate::render::{Canvas, VPoint, WorldTransform};

/// Draw every visible set of a graph.
pub fn draw_sets(canvas: &mut Canvas, graph: &Graph) {
    let wt = WorldTransform::new(graph);
    for set in &graph.sets {
        if set.hidden {
            continue;
        }
        draw_set_line(canvas, &wt, set);
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
