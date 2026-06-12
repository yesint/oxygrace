//! `.agr` writer: serialize a [`Project`] back to the Grace command language.
//!
//! The output mirrors Grace's own save layout (`putparms`, files.c): header →
//! page → font/color maps → defaults → timestamp → annotation objects →
//! per-graph parameter blocks → data sections. Every property is written
//! explicitly, defaults included, exactly like Grace — no "is this the
//! default?" logic — so output is diff-friendly and trivially complete.
//!
//! The file is always written as a *modern* project (`@version 50122` and the
//! current font map): the in-memory model is already past the reader's
//! version fixups (`postprocess_version`), so emitting as-modern is
//! self-consistent and old-version quirks are never reproduced.
//!
//! Known losses on a load → save round trip (inherent to the tolerant
//! reader, same as any re-save): command lines the reader does not model
//! (e.g. region blocks) are dropped.

use std::fmt::Write as _;
use std::path::Path;

use crate::color::DEFAULT_COLORMAP;
use crate::font::{FACE_NAMES, NUM_FONTS};
use crate::model::{
    Axis, BoxObj, FillType, Frame, Graph, GraphType, Legend, LineObj, LineType, Project,
    ScaleType, Set, SetType, StringObj, SymbolType, TickFormat, TickProps,
};

/// Serialize a project to Grace `.agr` command text.
pub fn save_str(project: &Project) -> String {
    let mut out = String::with_capacity(16 * 1024);
    write_header(&mut out, project);
    write_timestamp(&mut out, project);
    write_objects(&mut out, project);
    for (gno, graph) in project.graphs.iter().enumerate() {
        write_graph(&mut out, gno, graph);
    }
    for (gno, graph) in project.graphs.iter().enumerate() {
        write_data(&mut out, gno, graph);
    }
    out
}

/// Serialize a project to a `.agr` file on disk.
pub fn save<P: AsRef<Path>>(project: &Project, path: P) -> std::io::Result<()> {
    std::fs::write(path, save_str(project))
}

/// Shortest round-trip decimal form of a float (Rust's `Display` is lossless).
fn n(v: f64) -> String {
    format!("{v}")
}

/// Quote a string, escaping embedded double quotes like Grace's writer.
fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn onoff(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

fn truefalse(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

/// Placement side keyword (0 normal, 1 opposite, 2 both).
fn side(op: i32) -> &'static str {
    match op {
        1 => "opposite",
        2 => "both",
        _ => "normal",
    }
}

fn graph_type_kw(t: GraphType) -> &'static str {
    match t {
        GraphType::Xy => "XY",
        GraphType::Chart => "Chart",
        // Polar2 is not representable in the command language; "polar" is
        // the closest loadable spelling.
        GraphType::Polar | GraphType::Polar2 => "Polar",
        GraphType::Smith => "Smith",
        GraphType::Fixed => "Fixed",
        GraphType::Pie => "Pie",
    }
}

fn scale_kw(s: ScaleType) -> &'static str {
    match s {
        ScaleType::Normal => "Normal",
        ScaleType::Logarithmic => "Logarithmic",
        ScaleType::Reciprocal => "Reciprocal",
        ScaleType::Logit => "Logit",
    }
}

/// `@type` keyword for a set type (inverse of [`SetType::parse`]).
fn set_type_kw(t: SetType) -> &'static str {
    use SetType::*;
    match t {
        Xy => "xy",
        XyDx => "xydx",
        XyDy => "xydy",
        XyDxDx => "xydxdx",
        XyDyDy => "xydydy",
        XyDxDy => "xydxdy",
        XyDxDxDyDy => "xydxdxdydy",
        Bar => "bar",
        BarDy => "bardy",
        BarDyDy => "bardydy",
        XyHiLo => "xyhilo",
        Xyz => "xyz",
        XyR => "xyr",
        XySize => "xysize",
        XyColor => "xycolor",
        XyColPat => "xycolpat",
        XyVMap => "xyvmap",
        BoxPlot => "boxplot",
        Band => "band",
    }
}

/// `ticklabel format` keyword (inverse of [`TickFormat::parse`]).
fn tick_format_kw(f: TickFormat) -> &'static str {
    use TickFormat::*;
    match f {
        Decimal => "decimal",
        Exponential => "exponential",
        General => "general",
        Power => "power",
        Scientific => "scientific",
        Engineering => "engineering",
        Computing => "computing",
        DegreesLon => "degreeslon",
        DegreesLat => "degreeslat",
        MmDdYy => "mmddyy",
        DdMmYy => "ddmmyy",
        YyMmDd => "yymmdd",
        MmYy => "mmyy",
        MmDd => "mmdd",
        MonthDay => "monthday",
        DayMonth => "daymonth",
        Months => "months",
        MonthsY => "monthsy",
        MonthL => "monthl",
        DayOfWeekS => "dayofweeks",
        DayOfWeekL => "dayofweekl",
        DayOfYear => "dayofyear",
        Hms => "hms",
    }
}

fn symbol_code(s: SymbolType) -> i32 {
    use SymbolType::*;
    match s {
        None => 0,
        Circle => 1,
        Square => 2,
        Diamond => 3,
        TriangleUp => 4,
        TriangleLeft => 5,
        TriangleDown => 6,
        TriangleRight => 7,
        Plus => 8,
        Cross => 9,
        Star => 10,
        Char => 11,
    }
}

fn line_type_code(t: LineType) -> i32 {
    use LineType::*;
    match t {
        None => 0,
        Straight => 1,
        LeftStair => 2,
        RightStair => 3,
        Segment2 => 4,
        Segment3 => 5,
        IncrX => 6,
        DecrX => 7,
    }
}

fn fill_type_code(t: FillType) -> i32 {
    match t {
        FillType::None => 0,
        FillType::Polygon => 1,
        FillType::Baseline => 2,
    }
}

fn write_header(out: &mut String, p: &Project) {
    let _ = writeln!(out, "# Grace project file");
    let _ = writeln!(out, "# (written by oxygrace)");
    let _ = writeln!(out, "@version 50122");
    let _ = writeln!(out, "@page size {}, {}", p.page_width, p.page_height);
    for slot in 0..NUM_FONTS {
        let face = p.font_map[slot].clamp(0, NUM_FONTS as i32 - 1) as usize;
        let name = FACE_NAMES[face];
        let _ = writeln!(out, "@map font {slot} to \"{name}\", \"{name}\"");
    }
    write_colormap(out, p);
    let d = &p.defaults;
    let _ = writeln!(out, "@default linewidth {}", n(d.linewidth));
    let _ = writeln!(out, "@default linestyle {}", d.linestyle);
    let _ = writeln!(out, "@default color {}", d.color);
    let _ = writeln!(out, "@default pattern {}", d.pattern);
    let _ = writeln!(out, "@default font {}", d.font);
    let _ = writeln!(out, "@default char size {}", n(d.charsize));
    let _ = writeln!(out, "@default symbol size {}", n(d.symsize));
}

/// The 16 built-in colors patched by overrides, then any extra overrides in
/// index order — so the reloaded override list reproduces this exact block.
fn write_colormap(out: &mut String, p: &Project) {
    const NAMES: [&str; 16] = [
        "white", "black", "red", "green", "blue", "yellow", "brown", "grey", "violet", "cyan",
        "magenta", "orange", "indigo", "maroon", "turquoise", "green4",
    ];
    let lookup = |idx: i32| p.color_overrides.iter().find(|&&(i, _)| i == idx).map(|&(_, c)| c);
    for (i, &(r, g, b)) in DEFAULT_COLORMAP.iter().enumerate() {
        let (r, g, b) = lookup(i as i32).unwrap_or((r, g, b));
        let _ = writeln!(out, "@map color {i} to ({r}, {g}, {b}), \"{}\"", NAMES[i]);
    }
    let mut extra: Vec<i32> = p
        .color_overrides
        .iter()
        .map(|&(i, _)| i)
        .filter(|&i| !(0..16).contains(&i))
        .collect();
    extra.sort_unstable();
    extra.dedup();
    for i in extra {
        let (r, g, b) = lookup(i).unwrap();
        let _ = writeln!(out, "@map color {i} to ({r}, {g}, {b}), \"color{i}\"");
    }
}

fn write_timestamp(out: &mut String, p: &Project) {
    let t = &p.timestamp;
    let _ = writeln!(out, "@timestamp {}", onoff(t.active));
    let _ = writeln!(out, "@timestamp {}, {}", n(t.x), n(t.y));
    let _ = writeln!(out, "@timestamp color {}", t.color);
    let _ = writeln!(out, "@timestamp rot {}", n(t.rot));
    let _ = writeln!(out, "@timestamp font {}", t.font);
    let _ = writeln!(out, "@timestamp char size {}", n(t.charsize));
    let _ = writeln!(out, "@timestamp def {}", q(&t.text));
}

fn write_objects(out: &mut String, p: &Project) {
    for s in &p.strings {
        write_string_obj(out, s);
    }
    for l in &p.lines {
        write_line_obj(out, l);
    }
    for b in &p.boxes {
        write_boxlike(out, b, "box");
    }
    for e in &p.ellipses {
        write_boxlike(out, e, "ellipse");
    }
}

fn write_string_obj(out: &mut String, s: &StringObj) {
    let _ = writeln!(out, "@with string");
    let _ = writeln!(out, "@    string {}", onoff(s.active));
    let _ = writeln!(out, "@    string loctype {}", if s.loctype_view { "view" } else { "world" });
    let _ = writeln!(out, "@    string g{}", s.gno);
    let _ = writeln!(out, "@    string {}, {}", n(s.x), n(s.y));
    let _ = writeln!(out, "@    string color {}", s.color);
    let _ = writeln!(out, "@    string rot {}", n(s.rot));
    let _ = writeln!(out, "@    string font {}", s.font);
    let _ = writeln!(out, "@    string just {}", s.just);
    let _ = writeln!(out, "@    string char size {}", n(s.charsize));
    let _ = writeln!(out, "@string def {}", q(&s.text));
}

fn write_line_obj(out: &mut String, l: &LineObj) {
    let _ = writeln!(out, "@with line");
    let _ = writeln!(out, "@    line {}", onoff(l.active));
    let _ = writeln!(out, "@    line loctype {}", if l.loctype_view { "view" } else { "world" });
    let _ = writeln!(out, "@    line g{}", l.gno);
    let _ = writeln!(out, "@    line {}, {}, {}, {}", n(l.x1), n(l.y1), n(l.x2), n(l.y2));
    let _ = writeln!(out, "@    line linewidth {}", n(l.linewidth));
    let _ = writeln!(out, "@    line linestyle {}", l.linestyle);
    let _ = writeln!(out, "@    line color {}", l.color);
    let _ = writeln!(out, "@    line arrow {}", l.arrow_end);
    let _ = writeln!(out, "@    line arrow type {}", l.arrow_type);
    let _ = writeln!(out, "@    line arrow length {}", n(l.arrow_length));
    let _ = writeln!(out, "@    line arrow layout {}, {}", n(l.arrow_dl), n(l.arrow_ll));
    let _ = writeln!(out, "@line def");
}

fn write_boxlike(out: &mut String, b: &BoxObj, kind: &str) {
    let _ = writeln!(out, "@with {kind}");
    let _ = writeln!(out, "@    {kind} {}", onoff(b.active));
    let _ = writeln!(out, "@    {kind} loctype {}", if b.loctype_view { "view" } else { "world" });
    let _ = writeln!(out, "@    {kind} g{}", b.gno);
    let _ = writeln!(out, "@    {kind} {}, {}, {}, {}", n(b.x1), n(b.y1), n(b.x2), n(b.y2));
    let _ = writeln!(out, "@    {kind} linewidth {}", n(b.linewidth));
    let _ = writeln!(out, "@    {kind} linestyle {}", b.linestyle);
    let _ = writeln!(out, "@    {kind} color {}", b.color);
    let _ = writeln!(out, "@    {kind} fill color {}", b.fill_color);
    let _ = writeln!(out, "@    {kind} fill pattern {}", b.fill_pattern);
    let _ = writeln!(out, "@{kind} def");
}

fn write_graph(out: &mut String, gno: usize, g: &Graph) {
    let _ = writeln!(out, "@g{gno} {}", onoff(!g.hidden));
    let _ = writeln!(out, "@g{gno} hidden {}", truefalse(g.hidden));
    let _ = writeln!(out, "@g{gno} type {}", graph_type_kw(g.graph_type));
    let _ = writeln!(out, "@g{gno} stacked {}", truefalse(g.stacked));
    let _ = writeln!(out, "@g{gno} bar hgap {}", n(g.bargap));
    let _ = writeln!(out, "@with g{gno}");
    let w = &g.world;
    let _ = writeln!(out, "@    world {}, {}, {}, {}", n(w.xmin), n(w.ymin), n(w.xmax), n(w.ymax));
    let _ = writeln!(out, "@    znorm {}", n(g.znorm));
    let v = &g.view;
    let _ = writeln!(out, "@    view {}, {}, {}, {}", n(v.xmin), n(v.ymin), n(v.xmax), n(v.ymax));
    let l = &g.labels;
    let _ = writeln!(out, "@    title {}", q(&l.title));
    let _ = writeln!(out, "@    title font {}", l.title_font);
    let _ = writeln!(out, "@    title size {}", n(l.title_size));
    let _ = writeln!(out, "@    title color {}", l.title_color);
    let _ = writeln!(out, "@    subtitle {}", q(&l.subtitle));
    let _ = writeln!(out, "@    subtitle font {}", l.subtitle_font);
    let _ = writeln!(out, "@    subtitle size {}", n(l.subtitle_size));
    let _ = writeln!(out, "@    subtitle color {}", l.subtitle_color);
    let _ = writeln!(out, "@    xaxes scale {}", scale_kw(g.xscale));
    let _ = writeln!(out, "@    yaxes scale {}", scale_kw(g.yscale));
    let _ = writeln!(out, "@    xaxes invert {}", onoff(g.xinvert));
    let _ = writeln!(out, "@    yaxes invert {}", onoff(g.yinvert));
    for (name, axis) in [
        ("xaxis", &g.axes[0]),
        ("yaxis", &g.axes[1]),
        ("altxaxis", &g.axes[2]),
        ("altyaxis", &g.axes[3]),
    ] {
        write_axis(out, name, axis);
    }
    write_legend(out, &g.legend);
    write_frame(out, &g.frame);
    for (sno, set) in g.sets.iter().enumerate() {
        write_set(out, sno, set);
    }
}

fn write_axis(out: &mut String, name: &str, a: &Axis) {
    let mut w = |rest: &str| {
        let _ = writeln!(out, "@    {name}  {rest}");
    };
    // Unlike Grace (which stops after "off" for inactive axes), every field
    // is written even then, so the model round-trips field-for-field.
    w(onoff(a.active));
    w(&format!("type zero {}", truefalse(a.zero)));
    w(&format!("offset {} , {}", n(a.offs_normal), n(a.offs_opposite)));
    w(&format!("bar {}", onoff(a.draw_bar)));
    w(&format!("bar color {}", a.bar_color));
    w(&format!("bar linestyle {}", a.bar_linestyle));
    w(&format!("bar linewidth {}", n(a.bar_linewidth)));
    w(&format!("label {}", q(&a.label)));
    w(&format!("label layout {}", if a.label_perp { "perp" } else { "para" }));
    w(&format!("label op {}", side(a.label_op)));
    w(&format!("label char size {}", n(a.label_charsize)));
    w(&format!("label font {}", a.label_font));
    w(&format!("label color {}", a.label_color));
    w(&format!("tick {}", onoff(a.ticks)));
    w(&format!("tick major {}", n(a.major)));
    w(&format!("tick minor ticks {}", a.minor_ticks));
    w(&format!("tick default {}", a.autonum));
    w(&format!("tick place rounded {}", truefalse(a.tick_round)));
    w(match a.tick_inout {
        1 => "tick out",
        2 => "tick both",
        _ => "tick in",
    });
    w(&format!("tick op {}", side(a.op)));
    write_tick_level(out, name, "major", &a.major_props);
    write_tick_level(out, name, "minor", &a.minor_props);
    let mut w = |rest: &str| {
        let _ = writeln!(out, "@    {name}  {rest}");
    };
    w(&format!("ticklabel {}", onoff(a.ticklabels)));
    w(&format!("ticklabel format {}", tick_format_kw(a.tl_format)));
    w(&format!("ticklabel prec {}", a.tl_prec));
    w(&format!("ticklabel formula {}", q(&a.tl_formula)));
    w(&format!("ticklabel append {}", q(&a.tl_append)));
    w(&format!("ticklabel prepend {}", q(&a.tl_prepend)));
    w(&format!("ticklabel angle {}", a.tl_angle));
    w(&format!("ticklabel skip {}", a.tl_skip));
    w(&format!("ticklabel stagger {}", a.tl_stagger));
    w(&format!("ticklabel op {}", side(a.tl_op)));
    w(&format!(
        "ticklabel start type {}",
        if a.tl_start_spec { "spec" } else { "auto" }
    ));
    w(&format!("ticklabel start {}", n(a.tl_start)));
    w(&format!(
        "ticklabel stop type {}",
        if a.tl_stop_spec { "spec" } else { "auto" }
    ));
    w(&format!("ticklabel stop {}", n(a.tl_stop)));
    w(&format!("ticklabel char size {}", n(a.tl_charsize)));
    w(&format!("ticklabel font {}", a.tl_font));
    w(&format!("ticklabel color {}", a.tl_color));
    // Specified ticks last, as in Grace's save.
    w(&format!(
        "tick spec type {}",
        match a.spec_type {
            1 => "ticks",
            2 => "both",
            _ => "none",
        }
    ));
    if a.spec_type != 0 {
        w(&format!("tick spec {}", a.spec_count));
        for (i, t) in a.spec_ticks.iter().enumerate() {
            let level = if t.major { "major" } else { "minor" };
            w(&format!("tick {level} {i}, {}", n(t.pos)));
            if let Some(label) = &t.label {
                w(&format!("ticklabel {i}, {}", q(label)));
            }
        }
    }
}

fn write_tick_level(out: &mut String, name: &str, level: &str, p: &TickProps) {
    let _ = writeln!(out, "@    {name}  tick {level} size {}", n(p.size));
    let _ = writeln!(out, "@    {name}  tick {level} color {}", p.color);
    let _ = writeln!(out, "@    {name}  tick {level} linewidth {}", n(p.linewidth));
    let _ = writeln!(out, "@    {name}  tick {level} linestyle {}", p.linestyle);
    let _ = writeln!(out, "@    {name}  tick {level} grid {}", onoff(p.grid));
}

fn write_legend(out: &mut String, l: &Legend) {
    let mut w = |rest: &str| {
        let _ = writeln!(out, "@    legend {rest}");
    };
    w(onoff(l.active));
    w(&format!("loctype {}", if l.loctype_view { "view" } else { "world" }));
    w(&format!("{}, {}", n(l.x), n(l.y)));
    w(&format!("font {}", l.font));
    w(&format!("char size {}", n(l.charsize)));
    w(&format!("color {}", l.color));
    w(&format!("length {}", n(l.length)));
    w(&format!("vgap {}", n(l.vgap)));
    w(&format!("hgap {}", n(l.hgap)));
    w(&format!("invert {}", truefalse(l.invert)));
    w(&format!("box {}", onoff(l.box_on)));
    w(&format!("box color {}", l.box_color));
    w(&format!("box linewidth {}", n(l.box_linewidth)));
    w(&format!("box linestyle {}", l.box_linestyle));
    w(&format!("box fill color {}", l.box_fill_color));
    w(&format!("box fill pattern {}", l.box_fill_pattern));
}

fn write_frame(out: &mut String, f: &Frame) {
    let _ = writeln!(out, "@    frame type {}", f.frame_type);
    let _ = writeln!(out, "@    frame linestyle {}", f.linestyle);
    let _ = writeln!(out, "@    frame linewidth {}", n(f.linewidth));
    let _ = writeln!(out, "@    frame color {}", f.pen.color);
    let _ = writeln!(out, "@    frame pattern {}", f.pen.pattern);
    let _ = writeln!(out, "@    frame background color {}", f.fill_pen.color);
    // The reader derives `frame.fill` from a non-zero background pattern, so
    // keep that invariant on the way out.
    let pattern = if f.fill { f.fill_pen.pattern } else { 0 };
    let _ = writeln!(out, "@    frame background pattern {pattern}");
}

fn write_set(out: &mut String, sno: usize, s: &Set) {
    let mut w = |rest: &str| {
        let _ = writeln!(out, "@    s{sno} {rest}");
    };
    w(&format!("hidden {}", truefalse(s.hidden)));
    w(&format!("type {}", set_type_kw(s.set_type)));
    w(&format!("symbol {}", symbol_code(s.symbol)));
    w(&format!("symbol size {}", n(s.symbol_size)));
    w(&format!("symbol color {}", s.symbol_pen.color));
    w(&format!("symbol fill color {}", s.symbol_fill.color));
    w(&format!("symbol fill pattern {}", s.symbol_fill.pattern));
    w(&format!("symbol linewidth {}", n(s.symbol_linewidth)));
    w(&format!("symbol linestyle {}", s.symbol_linestyle));
    w(&format!("symbol char {}", s.symbol_char));
    w(&format!("symbol char font {}", s.symbol_char_font));
    w(&format!("symbol skip {}", s.symskip));
    w(&format!("line type {}", line_type_code(s.line_type)));
    w(&format!("line linestyle {}", s.linestyle));
    w(&format!("line linewidth {}", n(s.linewidth)));
    w(&format!("line color {}", s.line_pen.color));
    w(&format!("baseline type {}", s.baseline_type));
    w(&format!("dropline {}", onoff(s.dropline)));
    w(&format!("fill type {}", fill_type_code(s.fill_type)));
    w(&format!("fill rule {}", s.fill_rule));
    w(&format!("fill color {}", s.fill_pen.color));
    w(&format!("fill pattern {}", s.fill_pen.pattern));
    let a = &s.avalue;
    w(&format!("avalue {}", onoff(a.active)));
    w(&format!("avalue type {}", a.avtype));
    w(&format!("avalue char size {}", n(a.size)));
    w(&format!("avalue font {}", a.font));
    w(&format!("avalue color {}", a.color));
    w(&format!("avalue rot {}", n(a.angle)));
    w(&format!("avalue format {}", tick_format_kw(a.format)));
    w(&format!("avalue prec {}", a.prec));
    w(&format!("avalue prepend {}", q(&a.prepend)));
    w(&format!("avalue append {}", q(&a.append)));
    w(&format!("avalue offset {} , {}", n(a.offx), n(a.offy)));
    let e = &s.errbar;
    w(&format!("errorbar {}", onoff(e.active)));
    w(&format!("errorbar place {}", side(e.place)));
    w(&format!("errorbar color {}", e.color));
    w(&format!("errorbar size {}", n(e.size)));
    w(&format!("errorbar linewidth {}", n(e.linewidth)));
    w(&format!("errorbar linestyle {}", e.linestyle));
    w(&format!("errorbar riser linewidth {}", n(e.riser_linewidth)));
    w(&format!("errorbar riser linestyle {}", e.riser_linestyle));
    w(&format!("errorbar riser clip {}", onoff(e.arrow_clip)));
    w(&format!("errorbar riser clip length {}", n(e.cliplen)));
    w(&format!("comment {}", q(&s.comment)));
    w(&format!("legend {}", q(&s.legend)));
}

/// Data sections: `@target GN.SM` + `@type` + rows + `&`, like Grace.
fn write_data(out: &mut String, gno: usize, g: &Graph) {
    for (sno, set) in g.sets.iter().enumerate() {
        let _ = writeln!(out, "@target G{gno}.S{sno}");
        let _ = writeln!(out, "@type {}", set_type_kw(set.set_type));
        let npts = set.data.len();
        for i in 0..npts {
            let mut row = String::new();
            for col in &set.data.cols {
                if !row.is_empty() {
                    row.push(' ');
                }
                let _ = write!(row, "{}", n(col[i]));
            }
            if let Some(Some(s)) = set.data.strs.get(i) {
                let _ = write!(row, " {}", q(s));
            }
            let _ = writeln!(out, "{row}");
        }
        let _ = writeln!(out, "&");
    }
}
