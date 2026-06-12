//! Axis page: bar, ticks, tick labels and axis label — one page with
//! collapsible sections; the clicked sub-element's section opens expanded.

use oxygrace::model::TickFormat;
use oxygrace::Project;

use crate::edit::Edit;
use crate::inspector::{rows, SIDE_OPTS};

/// Which sub-element was clicked (drives which section starts open).
#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Bar,
    TickLabels,
    Label,
}

const INOUT_OPTS: [(i32, &str); 3] = [(0, "In"), (1, "Out"), (2, "Both")];

const FORMAT_OPTS: [(TickFormat, &str); 9] = [
    (TickFormat::Decimal, "Decimal"),
    (TickFormat::Exponential, "Exponential"),
    (TickFormat::General, "General"),
    (TickFormat::Power, "Power"),
    (TickFormat::Scientific, "Scientific"),
    (TickFormat::Engineering, "Engineering"),
    (TickFormat::Computing, "Computing"),
    (TickFormat::DegreesLon, "Degrees (lon)"),
    (TickFormat::DegreesLat, "Degrees (lat)"),
];

pub fn show(
    ui: &mut egui::Ui,
    project: &Project,
    g: usize,
    a: usize,
    focus: Focus,
    force: bool,
    edits: &mut Vec<Edit>,
) {
    let Some(axis) = project.graphs.get(g).and_then(|gr| gr.axes.get(a)) else {
        return;
    };
    // On the frame the selection changed, expand the clicked sub-element's
    // sections and fold the rest.
    let f = |want: bool| force.then_some(want);

    rows::section(ui, "Axis", focus == Focus::Bar, f(focus == Focus::Bar), "axis_main", |ui| {
        rows::toggle(ui, edits, "Active", axis.active, "axis: active", move |p, v| {
            p.graphs[g].axes[a].active = v;
        });
        rows::toggle(ui, edits, "At world zero", axis.zero, "axis: zero", move |p, v| {
            p.graphs[g].axes[a].zero = v;
        });
        rows::num(ui, edits, "Offset (normal)", axis.offs_normal, 0.005, "axis: offset", move |p, v| {
            p.graphs[g].axes[a].offs_normal = v;
        });
        rows::num(
            ui,
            edits,
            "Offset (opposite)",
            axis.offs_opposite,
            0.005,
            "axis: offset opposite",
            move |p, v| p.graphs[g].axes[a].offs_opposite = v,
        );
    });

    rows::section(ui, "Bar", focus == Focus::Bar, f(focus == Focus::Bar), "axis_bar", |ui| {
        rows::toggle(ui, edits, "Draw bar", axis.draw_bar, "axis bar: on", move |p, v| {
            p.graphs[g].axes[a].draw_bar = v;
        });
        rows::color(ui, edits, "Color", axis.bar_color, project, "axis bar: color", move |p, v| {
            p.graphs[g].axes[a].bar_color = v;
        });
        rows::linestyle(ui, edits, "Line style", axis.bar_linestyle, "axis bar: style", move |p, v| {
            p.graphs[g].axes[a].bar_linestyle = v;
        });
        rows::num(ui, edits, "Line width", axis.bar_linewidth, 0.1, "axis bar: width", move |p, v| {
            p.graphs[g].axes[a].bar_linewidth = v.max(0.0);
        });
    });

    rows::section(ui, "Ticks", focus == Focus::Bar, f(focus == Focus::Bar), "axis_ticks", |ui| {
        rows::toggle(ui, edits, "Draw ticks", axis.ticks, "ticks: on", move |p, v| {
            p.graphs[g].axes[a].ticks = v;
        });
        rows::num(ui, edits, "Major spacing", axis.major, 0.1, "ticks: major spacing", move |p, v| {
            p.graphs[g].axes[a].major = v;
        });
        rows::int(ui, edits, "Minor per major", axis.minor_ticks, 0..=99, "ticks: minor count", move |p, v| {
            p.graphs[g].axes[a].minor_ticks = v;
        });
        rows::combo(ui, edits, "Direction", axis.tick_inout, &INOUT_OPTS, "ticks: direction", move |p, v| {
            p.graphs[g].axes[a].tick_inout = v;
        });
        rows::combo(ui, edits, "Placement", axis.op, &SIDE_OPTS, "ticks: placement", move |p, v| {
            p.graphs[g].axes[a].op = v;
        });
        rows::toggle(ui, edits, "Round start", axis.tick_round, "ticks: rounded", move |p, v| {
            p.graphs[g].axes[a].tick_round = v;
        });
        let mj = axis.major_props;
        rows::num(ui, edits, "Major size", mj.size, 0.05, "ticks: major size", move |p, v| {
            p.graphs[g].axes[a].major_props.size = v;
        });
        rows::color(ui, edits, "Major color", mj.color, project, "ticks: major color", move |p, v| {
            p.graphs[g].axes[a].major_props.color = v;
        });
        rows::num(ui, edits, "Major width", mj.linewidth, 0.1, "ticks: major width", move |p, v| {
            p.graphs[g].axes[a].major_props.linewidth = v.max(0.0);
        });
        rows::toggle(ui, edits, "Major grid", mj.grid, "ticks: major grid", move |p, v| {
            p.graphs[g].axes[a].major_props.grid = v;
        });
        let mn = axis.minor_props;
        rows::num(ui, edits, "Minor size", mn.size, 0.05, "ticks: minor size", move |p, v| {
            p.graphs[g].axes[a].minor_props.size = v;
        });
        rows::color(ui, edits, "Minor color", mn.color, project, "ticks: minor color", move |p, v| {
            p.graphs[g].axes[a].minor_props.color = v;
        });
        rows::toggle(ui, edits, "Minor grid", mn.grid, "ticks: minor grid", move |p, v| {
            p.graphs[g].axes[a].minor_props.grid = v;
        });
    });

    rows::section(ui, "Tick labels", focus == Focus::TickLabels, f(focus == Focus::TickLabels), "axis_tl", |ui| {
        rows::toggle(ui, edits, "Show", axis.ticklabels, "tick labels: on", move |p, v| {
            p.graphs[g].axes[a].ticklabels = v;
        });
        rows::combo(ui, edits, "Format", axis.tl_format, &FORMAT_OPTS, "tick labels: format", move |p, v| {
            p.graphs[g].axes[a].tl_format = v;
        });
        rows::int(ui, edits, "Precision", axis.tl_prec, 0..=12, "tick labels: precision", move |p, v| {
            p.graphs[g].axes[a].tl_prec = v;
        });
        rows::num(ui, edits, "Char size", axis.tl_charsize, 0.05, "tick labels: size", move |p, v| {
            p.graphs[g].axes[a].tl_charsize = v.max(0.0);
        });
        rows::font(ui, edits, "Font", axis.tl_font, project, "tick labels: font", move |p, v| {
            p.graphs[g].axes[a].tl_font = v;
        });
        rows::color(ui, edits, "Color", axis.tl_color, project, "tick labels: color", move |p, v| {
            p.graphs[g].axes[a].tl_color = v;
        });
        rows::int(ui, edits, "Angle", axis.tl_angle, -360..=360, "tick labels: angle", move |p, v| {
            p.graphs[g].axes[a].tl_angle = v;
        });
        rows::int(ui, edits, "Skip", axis.tl_skip, 0..=20, "tick labels: skip", move |p, v| {
            p.graphs[g].axes[a].tl_skip = v;
        });
        rows::int(ui, edits, "Stagger", axis.tl_stagger, 0..=5, "tick labels: stagger", move |p, v| {
            p.graphs[g].axes[a].tl_stagger = v;
        });
        rows::text(ui, edits, "Prepend", &axis.tl_prepend, "tick labels: prepend", move |p, v| {
            p.graphs[g].axes[a].tl_prepend = v;
        });
        rows::text(ui, edits, "Append", &axis.tl_append, "tick labels: append", move |p, v| {
            p.graphs[g].axes[a].tl_append = v;
        });
        rows::text(ui, edits, "Formula ($t)", &axis.tl_formula, "tick labels: formula", move |p, v| {
            p.graphs[g].axes[a].tl_formula = v;
        });
        rows::combo(ui, edits, "Placement", axis.tl_op, &SIDE_OPTS, "tick labels: placement", move |p, v| {
            p.graphs[g].axes[a].tl_op = v;
        });
    });

    rows::section(ui, "Axis label", focus == Focus::Label, f(focus == Focus::Label), "axis_label", |ui| {
        rows::text(ui, edits, "Text", &axis.label, "axis label: text", move |p, v| {
            p.graphs[g].axes[a].label = v;
        });
        rows::toggle(ui, edits, "Perpendicular", axis.label_perp, "axis label: layout", move |p, v| {
            p.graphs[g].axes[a].label_perp = v;
        });
        rows::num(ui, edits, "Char size", axis.label_charsize, 0.05, "axis label: size", move |p, v| {
            p.graphs[g].axes[a].label_charsize = v.max(0.0);
        });
        rows::font(ui, edits, "Font", axis.label_font, project, "axis label: font", move |p, v| {
            p.graphs[g].axes[a].label_font = v;
        });
        rows::color(ui, edits, "Color", axis.label_color, project, "axis label: color", move |p, v| {
            p.graphs[g].axes[a].label_color = v;
        });
        rows::combo(ui, edits, "Placement", axis.label_op, &SIDE_OPTS, "axis label: placement", move |p, v| {
            p.graphs[g].axes[a].label_op = v;
        });
    });
}
