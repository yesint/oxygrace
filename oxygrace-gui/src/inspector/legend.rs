//! Legend page.

use oxygrace::Project;

use crate::edit::Edit;
use crate::inspector::rows;

const LOCTYPE_OPTS: [(bool, &str); 2] = [(true, "View"), (false, "World")];

pub fn show(ui: &mut egui::Ui, project: &Project, g: usize, edits: &mut Vec<Edit>) {
    let Some(l) = project.graphs.get(g).map(|gr| &gr.legend) else {
        return;
    };

    rows::section(ui, "Legend", true, None, "legend_main", |ui| {
        rows::toggle(ui, edits, "Show", l.active, "legend: on", move |p, v| {
            p.graphs[g].legend.active = v;
        });
        rows::combo(ui, edits, "Coordinates", l.loctype_view, &LOCTYPE_OPTS, "legend: loctype", move |p, v| {
            p.graphs[g].legend.loctype_view = v;
        });
        rows::num(ui, edits, "X", l.x, 0.005, "legend: x", move |p, v| {
            p.graphs[g].legend.x = v;
        });
        rows::num(ui, edits, "Y", l.y, 0.005, "legend: y", move |p, v| {
            p.graphs[g].legend.y = v;
        });
        rows::num(ui, edits, "Char size", l.charsize, 0.05, "legend: size", move |p, v| {
            p.graphs[g].legend.charsize = v.max(0.0);
        });
        rows::font(ui, edits, "Font", l.font, project, "legend: font", move |p, v| {
            p.graphs[g].legend.font = v;
        });
        rows::color(ui, edits, "Color", l.color, project, "legend: color", move |p, v| {
            p.graphs[g].legend.color = v;
        });
        rows::num(ui, edits, "Swatch length", l.length, 0.1, "legend: length", move |p, v| {
            p.graphs[g].legend.length = v.max(0.0);
        });
        rows::num(ui, edits, "V gap", l.vgap, 0.1, "legend: vgap", move |p, v| {
            p.graphs[g].legend.vgap = v;
        });
        rows::num(ui, edits, "H gap", l.hgap, 0.1, "legend: hgap", move |p, v| {
            p.graphs[g].legend.hgap = v;
        });
        rows::toggle(ui, edits, "Invert order", l.invert, "legend: invert", move |p, v| {
            p.graphs[g].legend.invert = v;
        });
    });

    rows::section(ui, "Box", true, None, "legend_box", |ui| {
        rows::toggle(ui, edits, "Draw box", l.box_on, "legend box: on", move |p, v| {
            p.graphs[g].legend.box_on = v;
        });
        rows::color(ui, edits, "Color", l.box_color, project, "legend box: color", move |p, v| {
            p.graphs[g].legend.box_color = v;
        });
        rows::linestyle(ui, edits, "Line style", l.box_linestyle, "legend box: style", move |p, v| {
            p.graphs[g].legend.box_linestyle = v;
        });
        rows::num(ui, edits, "Line width", l.box_linewidth, 0.1, "legend box: width", move |p, v| {
            p.graphs[g].legend.box_linewidth = v.max(0.0);
        });
        rows::color(ui, edits, "Fill color", l.box_fill_color, project, "legend box: fill color", move |p, v| {
            p.graphs[g].legend.box_fill_color = v;
        });
        rows::pattern(ui, edits, "Fill pattern", l.box_fill_pattern, "legend box: fill pattern", move |p, v| {
            p.graphs[g].legend.box_fill_pattern = v;
        });
    });
}
