//! Top-level draw orchestration, mirroring Grace's per-graph draw order
//! (`src/plotone.cpp`): frame fill → grid/data → axes+ticks → frame border →
//! titles. Legend and annotation objects are added in later milestones.

use crate::draw::{axes, decor, objects, pie, sets};
use crate::model::{Graph, Project};
use crate::render::{Canvas, HAlign, VAlign, VPoint};

/// Render an entire project onto a fresh canvas-backed pixmap.
pub fn draw_project(project: &Project, fonts: &crate::font::FontSet) -> Vec<u8> {
    let mut canvas = Canvas::new(project, fonts);
    for (i, graph) in project.graphs.iter().enumerate() {
        if !graph.hidden {
            draw_graph(&mut canvas, graph);
            // World-loctype annotation objects attached to this graph
            // (plotone.cpp: draw_objects(gno) at the end of plotone).
            objects::draw_objects(&mut canvas, project, objects::Pass::Graph { index: i, graph });
        }
    }
    // View-loctype objects are drawn once, after all graphs
    // (drawgraph: draw_objects(-1)), then the timestamp (draw_timestamp).
    objects::draw_objects(&mut canvas, project, objects::Pass::Page);
    objects::draw_timestamp(&mut canvas, project);
    canvas.to_png()
}

/// Draw one graph in Grace's layering order.
fn draw_graph(canvas: &mut Canvas, graph: &Graph) {
    fill_frame(canvas, graph);
    // Pie graphs draw the slices only: no axes, no legend (plotone skips
    // drawaxes/dolegend for GRAPH_PIE); frame and titles still apply.
    if graph.graph_type == crate::model::GraphType::Pie {
        pie::draw_pie(canvas, graph);
        draw_frame_border(canvas, graph);
        draw_titles(canvas, graph);
        return;
    }
    // Grid first (under everything: plotone calls drawgrid right after the
    // frame fill), then data, then axes, then the frame border on top.
    axes::draw_grid(canvas, graph);
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
    // A zero pen pattern makes the border invisible (Grace strokes the frame
    // with its Pen; pattern 0 = none) — xyz.agr hides its frame this way.
    if f.pen.pattern == 0 {
        return;
    }
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
