//! Annotation object pages: strings, lines, boxes/ellipses, timestamp.

use oxygrace::Project;

use crate::edit::Edit;
use crate::inspector::rows;

const LOCTYPE_OPTS: [(bool, &str); 2] = [(true, "View"), (false, "World")];

/// Grace justification bits: h = just & 3, v = just & 12 (`draw.h`).
const JUST_OPTS: [(i32, &str); 12] = [
    (0, "Left / baseline"),
    (1, "Right / baseline"),
    (2, "Center / baseline"),
    (4, "Left / bottom"),
    (5, "Right / bottom"),
    (6, "Center / bottom"),
    (8, "Left / top"),
    (9, "Right / top"),
    (10, "Center / top"),
    (12, "Left / middle"),
    (13, "Right / middle"),
    (14, "Center / middle"),
];

const ARROW_END_OPTS: [(i32, &str); 4] = [
    (0, "None"),
    (1, "At start"),
    (2, "At end"),
    (3, "Both ends"),
];

const ARROW_TYPE_OPTS: [(i32, &str); 3] = [(0, "Open"), (1, "Filled"), (2, "Background-filled")];

pub fn string(ui: &mut egui::Ui, project: &Project, i: usize, edits: &mut Vec<Edit>) {
    let Some(s) = project.strings.get(i) else { return };
    rows::section(ui, "String", true, None, "obj_string", |ui| {
        rows::toggle(ui, edits, "Active", s.active, "string: active", move |p, v| {
            p.strings[i].active = v;
        });
        rows::text(ui, edits, "Text", &s.text, "string: text", move |p, v| {
            p.strings[i].text = v;
        });
        rows::combo(ui, edits, "Coordinates", s.loctype_view, &LOCTYPE_OPTS, "string: loctype", move |p, v| {
            p.strings[i].loctype_view = v;
        });
        rows::num(ui, edits, "X", s.x, 0.005, "string: x", move |p, v| {
            p.strings[i].x = v;
        });
        rows::num(ui, edits, "Y", s.y, 0.005, "string: y", move |p, v| {
            p.strings[i].y = v;
        });
        rows::num(ui, edits, "Char size", s.charsize, 0.05, "string: size", move |p, v| {
            p.strings[i].charsize = v.max(0.0);
        });
        rows::font(ui, edits, "Font", s.font, project, "string: font", move |p, v| {
            p.strings[i].font = v;
        });
        rows::color(ui, edits, "Color", s.color, project, "string: color", move |p, v| {
            p.strings[i].color = v;
        });
        rows::num(ui, edits, "Rotation", s.rot, 1.0, "string: rotation", move |p, v| {
            p.strings[i].rot = v;
        });
        rows::combo(ui, edits, "Justification", s.just, &JUST_OPTS, "string: justification", move |p, v| {
            p.strings[i].just = v;
        });
    });
}

pub fn line(ui: &mut egui::Ui, project: &Project, i: usize, edits: &mut Vec<Edit>) {
    let Some(l) = project.lines.get(i) else { return };
    rows::section(ui, "Line", true, None, "obj_line", |ui| {
        rows::toggle(ui, edits, "Active", l.active, "line: active", move |p, v| {
            p.lines[i].active = v;
        });
        rows::combo(ui, edits, "Coordinates", l.loctype_view, &LOCTYPE_OPTS, "line: loctype", move |p, v| {
            p.lines[i].loctype_view = v;
        });
        rows::num(ui, edits, "X1", l.x1, 0.005, "line: x1", move |p, v| {
            p.lines[i].x1 = v;
        });
        rows::num(ui, edits, "Y1", l.y1, 0.005, "line: y1", move |p, v| {
            p.lines[i].y1 = v;
        });
        rows::num(ui, edits, "X2", l.x2, 0.005, "line: x2", move |p, v| {
            p.lines[i].x2 = v;
        });
        rows::num(ui, edits, "Y2", l.y2, 0.005, "line: y2", move |p, v| {
            p.lines[i].y2 = v;
        });
        rows::num(ui, edits, "Width", l.linewidth, 0.1, "line: width", move |p, v| {
            p.lines[i].linewidth = v.max(0.0);
        });
        rows::linestyle(ui, edits, "Style", l.linestyle, "line: style", move |p, v| {
            p.lines[i].linestyle = v;
        });
        rows::color(ui, edits, "Color", l.color, project, "line: color", move |p, v| {
            p.lines[i].color = v;
        });
    });
    rows::section(ui, "Arrows", true, None, "obj_line_arrows", |ui| {
        rows::combo(ui, edits, "Heads", l.arrow_end, &ARROW_END_OPTS, "line: arrow ends", move |p, v| {
            p.lines[i].arrow_end = v;
        });
        rows::combo(ui, edits, "Type", l.arrow_type, &ARROW_TYPE_OPTS, "line: arrow type", move |p, v| {
            p.lines[i].arrow_type = v;
        });
        rows::num(ui, edits, "Length", l.arrow_length, 0.05, "line: arrow length", move |p, v| {
            p.lines[i].arrow_length = v.max(0.0);
        });
    });
}

/// Box and ellipse share the model type; `ellipse` switches the vector.
fn pick(p: &mut Project, ellipse: bool, i: usize) -> &mut oxygrace::model::BoxObj {
    if ellipse {
        &mut p.ellipses[i]
    } else {
        &mut p.boxes[i]
    }
}

pub fn boxlike(ui: &mut egui::Ui, project: &Project, i: usize, ellipse: bool, edits: &mut Vec<Edit>) {
    let objs = if ellipse { &project.ellipses } else { &project.boxes };
    let Some(b) = objs.get(i) else { return };

    rows::section(ui, if ellipse { "Ellipse" } else { "Box" }, true, None, "obj_box", |ui| {
        rows::toggle(ui, edits, "Active", b.active, "box: active", move |p, v| {
            pick(p, ellipse, i).active = v;
        });
        rows::combo(ui, edits, "Coordinates", b.loctype_view, &LOCTYPE_OPTS, "box: loctype", move |p, v| {
            pick(p, ellipse, i).loctype_view = v;
        });
        rows::num(ui, edits, "X1", b.x1, 0.005, "box: x1", move |p, v| {
            pick(p, ellipse, i).x1 = v;
        });
        rows::num(ui, edits, "Y1", b.y1, 0.005, "box: y1", move |p, v| {
            pick(p, ellipse, i).y1 = v;
        });
        rows::num(ui, edits, "X2", b.x2, 0.005, "box: x2", move |p, v| {
            pick(p, ellipse, i).x2 = v;
        });
        rows::num(ui, edits, "Y2", b.y2, 0.005, "box: y2", move |p, v| {
            pick(p, ellipse, i).y2 = v;
        });
        rows::color(ui, edits, "Color", b.color, project, "box: color", move |p, v| {
            pick(p, ellipse, i).color = v;
        });
        rows::linestyle(ui, edits, "Line style", b.linestyle, "box: line style", move |p, v| {
            pick(p, ellipse, i).linestyle = v;
        });
        rows::num(ui, edits, "Line width", b.linewidth, 0.1, "box: line width", move |p, v| {
            pick(p, ellipse, i).linewidth = v.max(0.0);
        });
        rows::color(ui, edits, "Fill color", b.fill_color, project, "box: fill color", move |p, v| {
            pick(p, ellipse, i).fill_color = v;
        });
        rows::pattern(ui, edits, "Fill pattern", b.fill_pattern, "box: fill pattern", move |p, v| {
            pick(p, ellipse, i).fill_pattern = v;
        });
    });
}

pub fn timestamp(ui: &mut egui::Ui, project: &Project, edits: &mut Vec<Edit>) {
    let t = &project.timestamp;
    rows::section(ui, "Timestamp", true, None, "obj_timestamp", |ui| {
        rows::toggle(ui, edits, "Active", t.active, "timestamp: active", |p, v| {
            p.timestamp.active = v;
        });
        rows::text(ui, edits, "Text", &t.text, "timestamp: text", |p, v| {
            p.timestamp.text = v;
        });
        rows::num(ui, edits, "X", t.x, 0.005, "timestamp: x", |p, v| {
            p.timestamp.x = v;
        });
        rows::num(ui, edits, "Y", t.y, 0.005, "timestamp: y", |p, v| {
            p.timestamp.y = v;
        });
        rows::num(ui, edits, "Char size", t.charsize, 0.05, "timestamp: size", |p, v| {
            p.timestamp.charsize = v.max(0.0);
        });
        rows::font(ui, edits, "Font", t.font, project, "timestamp: font", |p, v| {
            p.timestamp.font = v;
        });
        rows::color(ui, edits, "Color", t.color, project, "timestamp: color", |p, v| {
            p.timestamp.color = v;
        });
    });
}
