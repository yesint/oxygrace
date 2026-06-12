//! Graph page: visibility/type, world window, viewport, scales, titles.

use oxygrace::model::{GraphType, ScaleType};
use oxygrace::Project;

use crate::edit::Edit;
use crate::inspector::rows;

const TYPE_OPTS: [(GraphType, &str); 5] = [
    (GraphType::Xy, "XY"),
    (GraphType::Chart, "Chart"),
    (GraphType::Polar, "Polar"),
    (GraphType::Fixed, "Fixed"),
    (GraphType::Pie, "Pie"),
];

const SCALE_OPTS: [(ScaleType, &str); 4] = [
    (ScaleType::Normal, "Linear"),
    (ScaleType::Logarithmic, "Logarithmic"),
    (ScaleType::Reciprocal, "Reciprocal"),
    (ScaleType::Logit, "Logit"),
];

/// Which part of the graph was clicked (drives which sections expand).
#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Area,
    Frame,
    Title,
    Subtitle,
}

pub fn show(
    ui: &mut egui::Ui,
    project: &Project,
    g: usize,
    focus: Focus,
    force: bool,
    edits: &mut Vec<Edit>,
) {
    let Some(graph) = project.graphs.get(g) else { return };
    // On the frame the selection changed, expand the clicked part's
    // sections and fold the rest.
    let f = |want: bool| force.then_some(want);

    rows::section(ui, "Graph", focus == Focus::Area, f(focus == Focus::Area), "graph_main", |ui| {
        rows::toggle(ui, edits, "Hidden", graph.hidden, "graph: hidden", move |p, v| {
            p.graphs[g].hidden = v;
        });
        rows::combo(ui, edits, "Type", graph.graph_type, &TYPE_OPTS, "graph: type", move |p, v| {
            p.graphs[g].graph_type = v;
        });
        rows::toggle(ui, edits, "Stacked", graph.stacked, "graph: stacked", move |p, v| {
            p.graphs[g].stacked = v;
        });
        rows::num(ui, edits, "Bar gap", graph.bargap, 0.01, "graph: bar gap", move |p, v| {
            p.graphs[g].bargap = v;
        });
    });

    rows::section(ui, "World", focus == Focus::Area, f(focus == Focus::Area), "graph_world", |ui| {
        let w = graph.world;
        rows::num(ui, edits, "X min", w.xmin, 0.1, "world: xmin", move |p, v| {
            p.graphs[g].world.xmin = v;
        });
        rows::num(ui, edits, "X max", w.xmax, 0.1, "world: xmax", move |p, v| {
            p.graphs[g].world.xmax = v;
        });
        rows::num(ui, edits, "Y min", w.ymin, 0.1, "world: ymin", move |p, v| {
            p.graphs[g].world.ymin = v;
        });
        rows::num(ui, edits, "Y max", w.ymax, 0.1, "world: ymax", move |p, v| {
            p.graphs[g].world.ymax = v;
        });
        rows::combo(ui, edits, "X scale", graph.xscale, &SCALE_OPTS, "graph: x scale", move |p, v| {
            p.graphs[g].xscale = v;
        });
        rows::combo(ui, edits, "Y scale", graph.yscale, &SCALE_OPTS, "graph: y scale", move |p, v| {
            p.graphs[g].yscale = v;
        });
        rows::toggle(ui, edits, "Invert X", graph.xinvert, "graph: invert x", move |p, v| {
            p.graphs[g].xinvert = v;
        });
        rows::toggle(ui, edits, "Invert Y", graph.yinvert, "graph: invert y", move |p, v| {
            p.graphs[g].yinvert = v;
        });
    });

    rows::section(ui, "Viewport", false, f(false), "graph_view", |ui| {
        let v = graph.view;
        rows::num(ui, edits, "X min", v.xmin, 0.005, "view: xmin", move |p, val| {
            p.graphs[g].view.xmin = val;
        });
        rows::num(ui, edits, "X max", v.xmax, 0.005, "view: xmax", move |p, val| {
            p.graphs[g].view.xmax = val;
        });
        rows::num(ui, edits, "Y min", v.ymin, 0.005, "view: ymin", move |p, val| {
            p.graphs[g].view.ymin = val;
        });
        rows::num(ui, edits, "Y max", v.ymax, 0.005, "view: ymax", move |p, val| {
            p.graphs[g].view.ymax = val;
        });
    });

    let fr = &graph.frame;
    rows::section(ui, "Frame", focus == Focus::Frame, f(focus == Focus::Frame), "graph_frame", |ui| {
        rows::int(ui, edits, "Frame type", fr.frame_type, 0..=6, "frame: type", move |p, v| {
            p.graphs[g].frame.frame_type = v;
        });
        rows::color(ui, edits, "Color", fr.pen.color, project, "frame: color", move |p, v| {
            p.graphs[g].frame.pen.color = v;
        });
        rows::pattern(ui, edits, "Pattern", fr.pen.pattern, "frame: pattern", move |p, v| {
            p.graphs[g].frame.pen.pattern = v;
        });
        rows::linestyle(ui, edits, "Line style", fr.linestyle, "frame: line style", move |p, v| {
            p.graphs[g].frame.linestyle = v;
        });
        rows::num(ui, edits, "Line width", fr.linewidth, 0.1, "frame: line width", move |p, v| {
            p.graphs[g].frame.linewidth = v.max(0.0);
        });
    });
    rows::section(
        ui,
        "Frame background",
        focus == Focus::Frame,
        f(focus == Focus::Frame),
        "graph_frame_bg",
        |ui| {
            // The model invariant (from the reader): `fill` is on iff the
            // fill pattern is non-zero — keep both in sync on toggle.
            rows::toggle(ui, edits, "Fill", fr.fill, "frame: background on", move |p, v| {
                let f = &mut p.graphs[g].frame;
                f.fill = v;
                if v && f.fill_pen.pattern == 0 {
                    f.fill_pen.pattern = 1;
                }
            });
            rows::color(ui, edits, "Color", fr.fill_pen.color, project, "frame: background color", move |p, v| {
                p.graphs[g].frame.fill_pen.color = v;
            });
            rows::pattern(ui, edits, "Pattern", fr.fill_pen.pattern, "frame: background pattern", move |p, v| {
                let f = &mut p.graphs[g].frame;
                f.fill_pen.pattern = v;
                f.fill = v != 0;
            });
        },
    );

    let l = &graph.labels;
    rows::section(ui, "Title", focus == Focus::Title, f(focus == Focus::Title), "graph_title", |ui| {
        rows::text(ui, edits, "Text", &l.title, "title: text", move |p, v| {
            p.graphs[g].labels.title = v;
        });
        rows::font(ui, edits, "Font", l.title_font, project, "title: font", move |p, v| {
            p.graphs[g].labels.title_font = v;
        });
        rows::num(ui, edits, "Size", l.title_size, 0.05, "title: size", move |p, v| {
            p.graphs[g].labels.title_size = v.max(0.0);
        });
        rows::color(ui, edits, "Color", l.title_color, project, "title: color", move |p, v| {
            p.graphs[g].labels.title_color = v;
        });
    });
    rows::section(ui, "Subtitle", focus == Focus::Subtitle, f(focus == Focus::Subtitle), "graph_subtitle", |ui| {
        rows::text(ui, edits, "Text", &l.subtitle, "subtitle: text", move |p, v| {
            p.graphs[g].labels.subtitle = v;
        });
        rows::font(ui, edits, "Font", l.subtitle_font, project, "subtitle: font", move |p, v| {
            p.graphs[g].labels.subtitle_font = v;
        });
        rows::num(ui, edits, "Size", l.subtitle_size, 0.05, "subtitle: size", move |p, v| {
            p.graphs[g].labels.subtitle_size = v.max(0.0);
        });
        rows::color(ui, edits, "Color", l.subtitle_color, project, "subtitle: color", move |p, v| {
            p.graphs[g].labels.subtitle_color = v;
        });
    });
}
