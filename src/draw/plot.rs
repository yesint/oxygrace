//! Top-level draw orchestration, mirroring Grace's per-graph draw order
//! (`src/plotone.cpp`): frame fill → grid/data → axes+ticks → frame border →
//! titles. Legend and annotation objects are added in later milestones.

use crate::draw::{axes, decor, objects, pie, sets};
use crate::model::{Graph, Project};
use crate::render::{Canvas, ElementId, HAlign, VAlign, VPoint};

/// Render an entire project to PNG bytes.
pub fn draw_project(project: &Project, fonts: &crate::font::FontSet) -> Vec<u8> {
    let mut canvas = Canvas::new(project, fonts);
    paint_project(&mut canvas, project);
    canvas.to_png()
}

/// Render an entire project to an SVG document. The same drawing code runs
/// against the SVG backend, so the output matches the raster rendering.
pub fn draw_project_svg(project: &Project, fonts: &crate::font::FontSet) -> String {
    let mut canvas = Canvas::new_svg(project, fonts);
    paint_project(&mut canvas, project);
    canvas.into_svg()
}

/// Render an entire project to a raw pixmap, recording element geometry for
/// hit-testing along the way.
pub fn draw_project_pixmap(
    project: &Project,
    fonts: &crate::font::FontSet,
) -> (tiny_skia::Pixmap, crate::render::RenderInfo) {
    let mut canvas = Canvas::new_recording(project, fonts);
    paint_project(&mut canvas, project);
    canvas.into_pixmap()
}

/// Paint a project onto a canvas (either backend).
fn paint_project(canvas: &mut Canvas, project: &Project) {
    for (i, graph) in project.graphs.iter().enumerate() {
        if !graph.hidden {
            draw_graph(canvas, i, graph);
            // World-loctype annotation objects attached to this graph
            // (plotone.cpp: draw_objects(gno) at the end of plotone).
            objects::draw_objects(canvas, project, objects::Pass::Graph { index: i, graph });
        }
    }
    // View-loctype objects are drawn once, after all graphs
    // (drawgraph: draw_objects(-1)), then the timestamp (draw_timestamp).
    objects::draw_objects(canvas, project, objects::Pass::Page);
    objects::draw_timestamp(canvas, project);
}

/// Draw one graph in Grace's layering order.
fn draw_graph(canvas: &mut Canvas, gno: usize, graph: &Graph) {
    // The whole graph is the hit-test fallback: an explicit viewport region
    // recorded first, so any element drawn later wins over it.
    canvas.push_element(ElementId::Graph(gno));
    let v = graph.view;
    canvas.record_rect_view(v.xmin, v.ymin, v.xmax, v.ymax);

    fill_frame(canvas, gno, graph);
    // Pie graphs draw the slices only: no axes, no legend (plotone skips
    // drawaxes/dolegend for GRAPH_PIE); frame and titles still apply.
    if graph.graph_type == crate::model::GraphType::Pie {
        // Pie data lives in set 0.
        canvas.push_element(ElementId::Set { graph: gno, set: 0 });
        pie::draw_pie(canvas, graph);
        canvas.pop_element();
        draw_frame_border(canvas, gno, graph);
        draw_titles(canvas, gno, graph);
        canvas.pop_element();
        return;
    }
    // Grid first (under everything: plotone calls drawgrid right after the
    // frame fill), then data, then axes, then the frame border on top.
    axes::draw_grid(canvas, gno, graph);
    sets::draw_sets(canvas, gno, graph);
    axes::draw_axes(canvas, gno, graph);
    draw_frame_border(canvas, gno, graph);
    decor::draw_legend(canvas, gno, graph);
    draw_titles(canvas, gno, graph);
    canvas.pop_element();
}

/// Fill the plotting area background if the frame requests it.
fn fill_frame(canvas: &mut Canvas, gno: usize, graph: &Graph) {
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
    canvas.push_element(ElementId::Frame(gno));
    canvas.fill_polygon(&rect, graph.frame.fill_pen.color, graph.frame.fill_pen.pattern);
    canvas.pop_element();
}

/// Draw the frame box around the plotting area (type 0 = closed rectangle).
fn draw_frame_border(canvas: &mut Canvas, gno: usize, graph: &Graph) {
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
    // The frame border doubles as the axis lines (axis bars are often off):
    // record each edge as the matching axis's ink so clicking "the axis"
    // selects it; the frame stays next in the click-cycle order.
    if graph.graph_type != crate::model::GraphType::Pie {
        let edge = |canvas: &mut Canvas, axis: usize, a: VPoint, b: VPoint| {
            canvas.push_element(ElementId::AxisBar { graph: gno, axis });
            canvas.record_polyline_view(&[a, b], f.linewidth);
            canvas.pop_element();
        };
        if graph.axes[0].active {
            edge(canvas, 0, rect[0], rect[1]); // bottom
            edge(canvas, 0, rect[3], rect[2]); // top
        }
        if graph.axes[1].active {
            edge(canvas, 1, rect[0], rect[3]); // left
            edge(canvas, 1, rect[1], rect[2]); // right
        }
    }
    canvas.push_element(ElementId::Frame(gno));
    canvas.draw_polyline(&rect, f.pen.color, f.linewidth, f.linestyle);
    canvas.pop_element();
}

/// Draw the title and subtitle above the frame.
fn draw_titles(canvas: &mut Canvas, gno: usize, graph: &Graph) {
    let v = graph.view;
    let l = &graph.labels;
    let cx = (v.xmin + v.xmax) / 2.0;
    if !l.title.is_empty() {
        canvas.push_element(ElementId::Title(gno));
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
        canvas.pop_element();
    }
    if !l.subtitle.is_empty() {
        canvas.push_element(ElementId::Subtitle(gno));
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
        canvas.pop_element();
    }
}
