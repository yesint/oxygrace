//! Top-level draw orchestration, mirroring Grace's per-graph draw order
//! (`src/plotone.cpp`): frame fill → grid/data → axes+ticks → frame border →
//! titles. Legend and annotation objects are added in later milestones.

use crate::draw::{axes, decor, sets};
use crate::model::{Graph, Project};
use crate::render::{Canvas, HAlign, VAlign, VPoint};

/// Render an entire project onto a fresh canvas-backed pixmap.
pub fn draw_project(project: &Project, fonts: &crate::font::FontSet) -> Vec<u8> {
    let mut canvas = Canvas::new(project, fonts);
    for graph in &project.graphs {
        if !graph.hidden {
            draw_graph(&mut canvas, graph);
        }
    }
    canvas.to_png()
}

/// Draw one graph in Grace's layering order.
fn draw_graph(canvas: &mut Canvas, graph: &Graph) {
    fill_frame(canvas, graph);
    // Grid lines are emitted inside draw_axes (before bars), then data, then
    // the ticks/labels and finally the frame border on top.
    sets::draw_sets(canvas, graph);
    axes::draw_axes(canvas, graph);
    draw_frame_border(canvas, graph);
    decor::draw_legend(canvas, graph);
    draw_titles(canvas, graph);
}

/// Fill the plotting area background if the frame requests it.
fn fill_frame(canvas: &mut Canvas, graph: &Graph) {
    if !graph.frame.fill || graph.frame.fill_pen.pattern == 0 {
        return;
    }
    let v = graph.view;
    let rect = [
        VPoint { x: v.xmin, y: v.ymin },
        VPoint { x: v.xmax, y: v.ymin },
        VPoint { x: v.xmax, y: v.ymax },
        VPoint { x: v.xmin, y: v.ymax },
    ];
    canvas.fill_polygon(&rect, graph.frame.fill_pen.color, graph.frame.fill_pen.pattern);
}

/// Draw the frame box around the plotting area (type 0 = closed rectangle).
fn draw_frame_border(canvas: &mut Canvas, graph: &Graph) {
    let f = &graph.frame;
    let v = graph.view;
    let rect = [
        VPoint { x: v.xmin, y: v.ymin },
        VPoint { x: v.xmax, y: v.ymin },
        VPoint { x: v.xmax, y: v.ymax },
        VPoint { x: v.xmin, y: v.ymax },
        VPoint { x: v.xmin, y: v.ymin },
    ];
    canvas.draw_polyline(&rect, f.pen.color, f.linewidth, f.linestyle);
}

/// Draw the title and subtitle above the frame.
fn draw_titles(canvas: &mut Canvas, graph: &Graph) {
    let v = graph.view;
    let l = &graph.labels;
    let cx = (v.xmin + v.xmax) / 2.0;
    if !l.title.is_empty() {
        canvas.draw_text(
            VPoint { x: cx, y: v.ymax + 0.06 },
            &l.title,
            l.title_size,
            l.title_font,
            l.title_color,
            HAlign::Center,
            VAlign::Baseline,
            0.0,
        );
    }
    if !l.subtitle.is_empty() {
        canvas.draw_text(
            VPoint { x: cx, y: v.ymax + 0.03 },
            &l.subtitle,
            l.subtitle_size,
            l.subtitle_font,
            l.subtitle_color,
            HAlign::Center,
            VAlign::Baseline,
            0.0,
        );
    }
}
