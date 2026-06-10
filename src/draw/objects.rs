//! Annotation objects: strings, lines (with arrowheads), boxes and ellipses.
//!
//! Literal port of QtGrace's `draw_objects` / `draw_box` / `draw_ellipse` /
//! `draw_line` / `draw_string` / `draw_arrowhead` (`plotone.cpp`). Objects are
//! drawn with clipping OFF. Grace calls `draw_objects(gno)` after each graph
//! (drawing the world-loctype objects attached to that graph) and
//! `draw_objects(-1)` after all graphs (drawing the view-loctype ones); the
//! draw order within a pass is boxes, ellipses, lines, strings.

use crate::model::{BoxObj, Graph, LineObj, Project, StringObj};
use crate::render::{Canvas, HAlign, VAlign, VPoint, WorldTransform};

/// Which pass we are drawing: a graph pass or the final page pass.
#[derive(Clone, Copy)]
pub enum Pass<'a> {
    /// After graph `index`: world-loctype objects attached to it.
    Graph { index: usize, graph: &'a Graph },
    /// After all graphs: view-loctype objects (Grace `draw_objects(-1)`).
    Page,
}

/// Draw all annotation objects belonging to the given pass.
pub fn draw_objects(canvas: &mut Canvas, project: &Project, pass: Pass) {
    // Resolve an object's anchor coordinates to view coordinates, or skip it
    // if it does not belong to this pass (draw_string/draw_line/draw_box all
    // share this gno/loctype filter).
    let to_view = |loctype_view: bool, gno: usize, x: f64, y: f64| -> Option<(f64, f64)> {
        match pass {
            Pass::Graph { index, graph } => {
                if loctype_view || gno != index {
                    None
                } else {
                    Some(WorldTransform::new(graph).world_to_view(x, y))
                }
            }
            Pass::Page => loctype_view.then_some((x, y)),
        }
    };

    for b in &project.boxes {
        if b.active {
            draw_box(canvas, b, &to_view);
        }
    }
    for e in &project.ellipses {
        if e.active {
            draw_ellipse(canvas, e, &to_view);
        }
    }
    for l in &project.lines {
        if l.active {
            draw_line(canvas, l, &to_view);
        }
    }
    for s in &project.strings {
        if s.active {
            draw_string(canvas, s, &to_view);
        }
    }
}

fn draw_box(
    canvas: &mut Canvas,
    b: &BoxObj,
    to_view: &impl Fn(bool, usize, f64, f64) -> Option<(f64, f64)>,
) {
    let (Some((x1, y1)), Some((x2, y2))) = (
        to_view(b.loctype_view, b.gno, b.x1, b.y1),
        to_view(b.loctype_view, b.gno, b.x2, b.y2),
    ) else {
        return;
    };
    let rect = [
        VPoint { x: x1, y: y1 },
        VPoint { x: x2, y: y1 },
        VPoint { x: x2, y: y2 },
        VPoint { x: x1, y: y2 },
    ];
    // Fill first, then the border (plotone.cpp draw_box).
    if b.fill_pattern != 0 {
        canvas.fill_polygon(&rect, b.fill_color, b.fill_pattern);
    }
    if b.linestyle != 0 {
        let mut closed = rect.to_vec();
        closed.push(rect[0]);
        canvas.draw_polyline(&closed, b.color, b.linewidth, b.linestyle);
    }
}

fn draw_ellipse(
    canvas: &mut Canvas,
    e: &BoxObj,
    to_view: &impl Fn(bool, usize, f64, f64) -> Option<(f64, f64)>,
) {
    let (Some((x1, y1)), Some((x2, y2))) = (
        to_view(e.loctype_view, e.gno, e.x1, e.y1),
        to_view(e.loctype_view, e.gno, e.x2, e.y2),
    ) else {
        return;
    };
    let (p1, p2) = (VPoint { x: x1, y: y1 }, VPoint { x: x2, y: y2 });
    if e.fill_pattern != 0 {
        canvas.fill_ellipse(p1, p2, e.fill_color, e.fill_pattern);
    }
    if e.linestyle != 0 {
        canvas.stroke_ellipse(p1, p2, e.color, e.linewidth, e.linestyle);
    }
}

fn draw_line(
    canvas: &mut Canvas,
    l: &LineObj,
    to_view: &impl Fn(bool, usize, f64, f64) -> Option<(f64, f64)>,
) {
    let (Some((x1, y1)), Some((x2, y2))) = (
        to_view(l.loctype_view, l.gno, l.x1, l.y1),
        to_view(l.loctype_view, l.gno, l.x2, l.y2),
    ) else {
        return;
    };
    let vp1 = VPoint { x: x1, y: y1 };
    let vp2 = VPoint { x: x2, y: y2 };
    canvas.draw_polyline(&[vp1, vp2], l.color, l.linewidth, l.linestyle);
    // Arrowheads: 0 none, 1 at start, 2 at end, 3 both (plotone.cpp draw_line).
    if l.arrow_end == 1 || l.arrow_end == 3 {
        draw_arrowhead(canvas, vp2, vp1, l);
    }
    if l.arrow_end == 2 || l.arrow_end == 3 {
        draw_arrowhead(canvas, vp1, vp2, l);
    }
}

/// Arrowhead at `vp2`, pointing away from `vp1` (plotone.cpp `draw_arrowhead`).
fn draw_arrowhead(canvas: &mut Canvas, vp1: VPoint, vp2: VPoint, l: &LineObj) {
    let vlength = ((vp2.x - vp1.x).powi(2) + (vp2.y - vp1.y).powi(2)).sqrt();
    if vlength == 0.0 {
        return;
    }
    let nx = (vp2.x - vp1.x) / vlength;
    let ny = (vp2.y - vp1.y) / vlength;

    // L = 0.01 * length; d and l are the layout form factors times L.
    let big_l = 0.01 * l.arrow_length;
    let d = big_l * l.arrow_dl;
    let ll = big_l * l.arrow_ll;

    let mut vpc = VPoint {
        x: vp2.x - big_l * nx,
        y: vp2.y - big_l * ny,
    };
    let vpl = VPoint {
        x: vpc.x + 0.5 * d * ny,
        y: vpc.y - 0.5 * d * nx,
    };
    let vpr = VPoint {
        x: vpc.x - 0.5 * d * ny,
        y: vpc.y + 0.5 * d * nx,
    };
    vpc.x += ll * nx;
    vpc.y += ll * ny;

    let vps = [vpl, vp2, vpr, vpc];
    // Arrowheads are always drawn with solid lines (setlinestyle(1)).
    match l.arrow_type {
        // Open: just the two head strokes.
        0 => canvas.draw_polyline(&vps[..3], l.color, l.linewidth, 1),
        // Filled with the line color; type 2 fills with the background color.
        1 | 2 => {
            let fill_color = if l.arrow_type == 2 { 0 } else { l.color };
            canvas.fill_polygon(&vps, fill_color, 1);
            let mut closed = vps.to_vec();
            closed.push(vps[0]);
            canvas.draw_polyline(&closed, l.color, l.linewidth, 1);
        }
        _ => {}
    }
}

/// Draw the page timestamp (plotone.cpp `draw_timestamp`): a plain string at
/// view coordinates. Grace refreshes the text to the render time; we keep the
/// text stored in the file so output is reproducible.
pub fn draw_timestamp(canvas: &mut Canvas, project: &Project) {
    let t = &project.timestamp;
    if !t.active || t.text.is_empty() {
        return;
    }
    canvas.draw_text(
        VPoint { x: t.x, y: t.y },
        &t.text,
        t.charsize,
        t.font,
        t.color,
        HAlign::Left,
        VAlign::Baseline,
        t.rot,
    );
}

fn draw_string(
    canvas: &mut Canvas,
    s: &StringObj,
    to_view: &impl Fn(bool, usize, f64, f64) -> Option<(f64, f64)>,
) {
    if s.text.is_empty() || s.charsize <= 0.0 {
        return;
    }
    let Some((x, y)) = to_view(s.loctype_view, s.gno, s.x, s.y) else {
        return;
    };
    // Grace justification bits (draw.h): h = just & 3, v = just & 12.
    let halign = match s.just & 3 {
        1 => HAlign::Right,
        2 => HAlign::Center,
        _ => HAlign::Left,
    };
    let valign = match s.just & 12 {
        4 => VAlign::Bottom,
        8 => VAlign::Top,
        12 => VAlign::Middle,
        _ => VAlign::Baseline,
    };
    canvas.draw_text(
        VPoint { x, y },
        &s.text,
        s.charsize,
        s.font,
        s.color,
        halign,
        valign,
        s.rot,
    );
}
