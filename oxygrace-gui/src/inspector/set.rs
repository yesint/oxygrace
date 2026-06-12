//! Set (dataset) page: line, symbol, fill, error bars, value labels.

use oxygrace::model::{FillType, LineType, SetType, SymbolType};
use oxygrace::Project;

use crate::edit::Edit;
use crate::inspector::{rows, SIDE_OPTS};

const TYPE_OPTS: [(SetType, &str); 19] = [
    (SetType::Xy, "xy"),
    (SetType::XyDx, "xydx"),
    (SetType::XyDy, "xydy"),
    (SetType::XyDxDx, "xydxdx"),
    (SetType::XyDyDy, "xydydy"),
    (SetType::XyDxDy, "xydxdy"),
    (SetType::XyDxDxDyDy, "xydxdxdydy"),
    (SetType::Bar, "bar"),
    (SetType::BarDy, "bardy"),
    (SetType::BarDyDy, "bardydy"),
    (SetType::XyHiLo, "xyhilo"),
    (SetType::Xyz, "xyz"),
    (SetType::XyR, "xyr"),
    (SetType::XySize, "xysize"),
    (SetType::XyColor, "xycolor"),
    (SetType::XyColPat, "xycolpat"),
    (SetType::XyVMap, "xyvmap"),
    (SetType::BoxPlot, "boxplot"),
    (SetType::Band, "band"),
];

const LINE_TYPE_OPTS: [(LineType, &str); 8] = [
    (LineType::None, "None"),
    (LineType::Straight, "Straight"),
    (LineType::LeftStair, "Left stairs"),
    (LineType::RightStair, "Right stairs"),
    (LineType::Segment2, "Segments (pairs)"),
    (LineType::Segment3, "Segments (triples)"),
    (LineType::IncrX, "Increasing X only"),
    (LineType::DecrX, "Decreasing X only"),
];

const SYMBOL_OPTS: [(SymbolType, &str); 12] = [
    (SymbolType::None, "None"),
    (SymbolType::Circle, "Circle"),
    (SymbolType::Square, "Square"),
    (SymbolType::Diamond, "Diamond"),
    (SymbolType::TriangleUp, "Triangle up"),
    (SymbolType::TriangleLeft, "Triangle left"),
    (SymbolType::TriangleDown, "Triangle down"),
    (SymbolType::TriangleRight, "Triangle right"),
    (SymbolType::Plus, "Plus"),
    (SymbolType::Cross, "Cross"),
    (SymbolType::Star, "Star"),
    (SymbolType::Char, "Character"),
];

const FILL_OPTS: [(FillType, &str); 3] = [
    (FillType::None, "None"),
    (FillType::Polygon, "As polygon"),
    (FillType::Baseline, "To baseline"),
];

pub fn show(ui: &mut egui::Ui, project: &Project, g: usize, s: usize, edits: &mut Vec<Edit>) {
    let Some(set) = project.graphs.get(g).and_then(|gr| gr.sets.get(s)) else {
        return;
    };
    ui.weak(format!("{} points", set.data.len()));

    rows::section(ui, "Set", true, None, "set_main", |ui| {
        rows::toggle(ui, edits, "Hidden", set.hidden, "set: hidden", move |p, v| {
            p.graphs[g].sets[s].hidden = v;
        });
        rows::combo(ui, edits, "Type", set.set_type, &TYPE_OPTS, "set: type", move |p, v| {
            p.graphs[g].sets[s].set_type = v;
        });
        rows::text(ui, edits, "Legend", &set.legend, "set: legend", move |p, v| {
            p.graphs[g].sets[s].legend = v;
        });
    });

    rows::section(ui, "Line", true, None, "set_line", |ui| {
        rows::combo(ui, edits, "Type", set.line_type, &LINE_TYPE_OPTS, "set line: type", move |p, v| {
            p.graphs[g].sets[s].line_type = v;
        });
        rows::linestyle(ui, edits, "Style", set.linestyle, "set line: style", move |p, v| {
            p.graphs[g].sets[s].linestyle = v;
        });
        rows::num(ui, edits, "Width", set.linewidth, 0.1, "set line: width", move |p, v| {
            p.graphs[g].sets[s].linewidth = v.max(0.0);
        });
        rows::color(ui, edits, "Color", set.line_pen.color, project, "set line: color", move |p, v| {
            p.graphs[g].sets[s].line_pen.color = v;
        });
        rows::toggle(ui, edits, "Drop lines", set.dropline, "set: drop lines", move |p, v| {
            p.graphs[g].sets[s].dropline = v;
        });
    });

    rows::section(ui, "Symbol", true, None, "set_symbol", |ui| {
        rows::combo(ui, edits, "Type", set.symbol, &SYMBOL_OPTS, "symbol: type", move |p, v| {
            p.graphs[g].sets[s].symbol = v;
        });
        rows::num(ui, edits, "Size", set.symbol_size, 0.05, "symbol: size", move |p, v| {
            p.graphs[g].sets[s].symbol_size = v.max(0.0);
        });
        rows::color(ui, edits, "Color", set.symbol_pen.color, project, "symbol: color", move |p, v| {
            p.graphs[g].sets[s].symbol_pen.color = v;
        });
        rows::color(ui, edits, "Fill color", set.symbol_fill.color, project, "symbol: fill color", move |p, v| {
            p.graphs[g].sets[s].symbol_fill.color = v;
        });
        rows::pattern(ui, edits, "Fill pattern", set.symbol_fill.pattern, "symbol: fill pattern", move |p, v| {
            p.graphs[g].sets[s].symbol_fill.pattern = v;
        });
        rows::num(ui, edits, "Outline width", set.symbol_linewidth, 0.1, "symbol: width", move |p, v| {
            p.graphs[g].sets[s].symbol_linewidth = v.max(0.0);
        });
        rows::int(ui, edits, "Skip", set.symskip, 0..=999, "symbol: skip", move |p, v| {
            p.graphs[g].sets[s].symskip = v;
        });
        if set.symbol == SymbolType::Char {
            rows::int(ui, edits, "Char code", set.symbol_char as i32, 0..=255, "symbol: char", move |p, v| {
                p.graphs[g].sets[s].symbol_char = v.clamp(0, 255) as u8;
            });
            rows::font(ui, edits, "Char font", set.symbol_char_font, project, "symbol: char font", move |p, v| {
                p.graphs[g].sets[s].symbol_char_font = v;
            });
        }
    });

    rows::section(ui, "Fill", false, None, "set_fill", |ui| {
        rows::combo(ui, edits, "Type", set.fill_type, &FILL_OPTS, "fill: type", move |p, v| {
            p.graphs[g].sets[s].fill_type = v;
        });
        rows::color(ui, edits, "Color", set.fill_pen.color, project, "fill: color", move |p, v| {
            p.graphs[g].sets[s].fill_pen.color = v;
        });
        rows::pattern(ui, edits, "Pattern", set.fill_pen.pattern, "fill: pattern", move |p, v| {
            p.graphs[g].sets[s].fill_pen.pattern = v;
        });
        rows::int(ui, edits, "Baseline type", set.baseline_type, 0..=4, "fill: baseline", move |p, v| {
            p.graphs[g].sets[s].baseline_type = v;
        });
    });

    let e = &set.errbar;
    rows::section(ui, "Error bars", false, None, "set_errbar", |ui| {
        rows::toggle(ui, edits, "Show", e.active, "error bars: on", move |p, v| {
            p.graphs[g].sets[s].errbar.active = v;
        });
        rows::combo(ui, edits, "Placement", e.place, &SIDE_OPTS, "error bars: placement", move |p, v| {
            p.graphs[g].sets[s].errbar.place = v;
        });
        rows::color(ui, edits, "Color", e.color, project, "error bars: color", move |p, v| {
            p.graphs[g].sets[s].errbar.color = v;
        });
        rows::num(ui, edits, "Cap size", e.size, 0.05, "error bars: size", move |p, v| {
            p.graphs[g].sets[s].errbar.size = v.max(0.0);
        });
        rows::num(ui, edits, "Line width", e.linewidth, 0.1, "error bars: width", move |p, v| {
            p.graphs[g].sets[s].errbar.linewidth = v.max(0.0);
        });
        rows::num(ui, edits, "Riser width", e.riser_linewidth, 0.1, "error bars: riser width", move |p, v| {
            p.graphs[g].sets[s].errbar.riser_linewidth = v.max(0.0);
        });
    });

    let a = &set.avalue;
    rows::section(ui, "Value labels", false, None, "set_avalue", |ui| {
        rows::toggle(ui, edits, "Show", a.active, "value labels: on", move |p, v| {
            p.graphs[g].sets[s].avalue.active = v;
        });
        rows::int(ui, edits, "Type (0-5)", a.avtype, 0..=5, "value labels: type", move |p, v| {
            p.graphs[g].sets[s].avalue.avtype = v;
        });
        rows::num(ui, edits, "Char size", a.size, 0.05, "value labels: size", move |p, v| {
            p.graphs[g].sets[s].avalue.size = v.max(0.0);
        });
        rows::font(ui, edits, "Font", a.font, project, "value labels: font", move |p, v| {
            p.graphs[g].sets[s].avalue.font = v;
        });
        rows::color(ui, edits, "Color", a.color, project, "value labels: color", move |p, v| {
            p.graphs[g].sets[s].avalue.color = v;
        });
        rows::int(ui, edits, "Precision", a.prec, 0..=12, "value labels: precision", move |p, v| {
            p.graphs[g].sets[s].avalue.prec = v;
        });
    });
}
