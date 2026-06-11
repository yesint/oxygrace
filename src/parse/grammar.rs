//! PEG grammar for the Grace `@`-command language.
//!
//! Each input line (with the leading `@` already removed) parses into one
//! [`Command`]. The [`reader`](crate::parse::reader) then applies commands to
//! the model using a small parse-state cursor (current graph / set / axis).
//!
//! The grammar covers the milestone-1 command subset and is deliberately
//! tolerant: any line it does not recognize parses as [`Command::Unknown`]
//! rather than failing, so real-world `.agr` files load without aborting.
//!
//! Keywords are matched case-insensitively (Grace treats `@WITH` and `@with`
//! identically); quoted strings preserve their case.

use crate::model::{AxisId, GraphType, ScaleType, SetType, TickFormat};

/// Which world/view bound a component-form command sets.
#[derive(Debug, Clone, Copy)]
pub enum Bound {
    Xmin,
    Xmax,
    Ymin,
    Ymax,
}

/// A `@world` command: full 4-tuple or a single component.
#[derive(Debug, Clone, Copy)]
pub enum WorldSpec {
    Full(f64, f64, f64, f64),
    Component(Bound, f64),
}

/// A `@view` command: full 4-tuple or a single component.
#[derive(Debug, Clone, Copy)]
pub enum ViewSpec {
    Full(f64, f64, f64, f64),
    Component(Bound, f64),
}

/// A `@default ...` property.
#[derive(Debug, Clone, Copy)]
pub enum DefaultProp {
    Linestyle(i32),
    Linewidth(f64),
    Color(i32),
    Pattern(i32),
    CharSize(f64),
    Font(i32),
    SymbolSize(f64),
}

/// A property of one tick level (major/minor) within an axis command.
#[derive(Debug, Clone, Copy)]
pub enum TickLevelProp {
    Spacing(f64), // major only
    Size(f64),
    Color(i32),
    Linewidth(f64),
    Linestyle(i32),
    Grid(bool),
}

/// A property set by an `@<axis> ...` command.
#[derive(Debug, Clone)]
pub enum AxisProp {
    Active(bool),
    BarActive(bool),
    BarColor(i32),
    BarLinestyle(i32),
    BarLinewidth(f64),
    LabelText(String),
    LabelFont(i32),
    LabelColor(i32),
    LabelCharSize(f64),
    TicksActive(bool),
    /// Tick direction: 0 in, 1 out, 2 both (`tick in|out|both`).
    TicksInOut(i32),
    MinorTicks(i32),
    AutoNum(i32),
    TickRound(bool),
    TickOp(i32),
    TlOp(i32),
    LabelOp(i32),
    LabelPerp(bool),
    Zero(bool),
    Offset(f64, f64),
    Major(TickLevelProp),
    Minor(TickLevelProp),
    TlActive(bool),
    TlPrec(i32),
    TlFormat(TickFormat),
    TlFont(i32),
    TlColor(i32),
    TlCharSize(f64),
    TlAngle(i32),
    TlAppend(String),
    TlPrepend(String),
    TlSkip(i32),
    TlStagger(i32),
    TlFormula(String),
    /// `tick spec type none|ticks|both` -> 0/1/2 (also the old
    /// `tick type spec` / `ticklabel type spec` forms).
    SpecType(i32),
    /// Old `tick type spec`: positions are specified (keep BOTH if set).
    SpecMarksOld,
    /// Old `ticklabel type auto`: drop BOTH back to MARKS.
    SpecLabelsAutoOld,
    SpecCount(usize),
    SpecPos { idx: usize, pos: f64, major: bool },
    SpecLabel { idx: usize, label: String },
    TlStartSpec(bool),
    TlStart(f64),
    TlStopSpec(bool),
    TlStop(f64),
    Ignored,
}

/// A property set by an `@s<n> ...` command.
#[derive(Debug, Clone)]
pub enum SetProp {
    Type(SetType),
    Hidden(bool),
    Symbol(i32),
    SymbolSize(f64),
    SymbolColor(i32),
    SymbolFillColor(i32),
    SymbolFillPattern(i32),
    SymbolLinewidth(f64),
    SymbolChar(i32),
    SymbolSkip(i32),
    SymbolCharFont(i32),
    SymbolLinestyle(i32),
    LineType(i32),
    LineColor(i32),
    LineLinewidth(f64),
    LineLinestyle(i32),
    Color(i32),      // legacy 4.x form
    Linewidth(f64),  // legacy
    Linestyle(i32),  // legacy
    FillType(i32),
    FillRule(i32),
    FillColor(i32),
    FillPattern(i32),
    BaselineType(i32),
    Dropline(bool),
    Legend(String),
    Comment(String),
    AvOn(bool),
    AvType(i32),
    AvSize(f64),
    AvFont(i32),
    AvColor(i32),
    AvRot(f64),
    AvFormat(TickFormat),
    AvPrec(i32),
    AvOffset(f64, f64),
    AvPrepend(String),
    AvAppend(String),
    EbOn(bool),
    EbPlace(i32),
    EbColor(i32),
    EbSize(f64),
    EbLinewidth(f64),
    EbLinestyle(i32),
    EbRiserLinewidth(f64),
    EbRiserLinestyle(i32),
    EbRiserClip(bool),
    EbRiserClipLen(f64),
    Ignored,
}

/// A property set by an `@frame ...` command.
#[derive(Debug, Clone, Copy)]
pub enum FrameProp {
    Type(i32),
    Linestyle(i32),
    Linewidth(f64),
    Color(i32),
    Pattern(i32),
    BackgroundColor(i32),
    BackgroundPattern(i32),
}

/// A property set by `@title`/`@subtitle`.
#[derive(Debug, Clone)]
pub enum TextProp {
    Text(String),
    Font(i32),
    Size(f64),
    Color(i32),
}

/// A property set by `@legend ...`.
#[derive(Debug, Clone, Copy)]
pub enum LegendProp {
    Active(bool),
    LoctypeView(bool),
    Position(f64, f64),
    X(f64),
    Y(f64),
    Font(i32),
    Color(i32),
    CharSize(f64),
    Length(f64),
    Vgap(f64),
    Hgap(f64),
    Invert(bool),
    BoxOn(bool),
    BoxColor(i32),
    BoxLinewidth(f64),
    BoxLinestyle(i32),
    BoxFillColor(i32),
    BoxFillPattern(i32),
    Ignored,
}

/// Kind of annotation object opened by `@with string|line|box|ellipse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjKind {
    String,
    Line,
    Box,
    Ellipse,
}

/// A property line of the annotation object currently being defined.
#[derive(Debug, Clone)]
pub enum ObjProp {
    On(bool),
    /// `loctype view` (true) / `loctype world` (false).
    LoctypeView(bool),
    /// `string g0` — attach to a graph (for world loctype).
    Graph(usize),
    /// `string 0.5, 0.2` — anchor point.
    Pos2(f64, f64),
    /// `line/box/ellipse x1, y1, x2, y2`.
    Pos4(f64, f64, f64, f64),
    Color(i32),
    Linewidth(f64),
    Linestyle(i32),
    Rot(f64),
    Font(i32),
    Just(i32),
    CharSize(f64),
    FillColor(i32),
    FillPattern(i32),
    /// `arrow 0..3` — which line ends carry an arrowhead.
    ArrowEnd(i32),
    ArrowType(i32),
    /// `arrow length 1.0` (new) / `arrow size 1.0` (old format).
    ArrowLength(f64),
    /// `arrow layout d, l` — d/L and l/L form factors.
    ArrowLayout(f64, f64),
    /// `@string def "text"` — the string text (also activates it).
    Def(String),
    /// `@line def` / `@box def` / `@ellipse def` end-of-definition marker.
    EndDef,
    Ignored,
}

/// One parsed command from a `.agr` line.
#[derive(Debug, Clone)]
pub enum Command {
    Version(i32),
    /// `@with string|line|box|ellipse` — open a new annotation object.
    WithObject(ObjKind),
    /// `@ <kind> <prop>` while an object definition is open.
    Object { kind: ObjKind, prop: ObjProp },
    /// `@timestamp <prop>` — the page timestamp string.
    Timestamp(ObjProp),
    /// `@page size W H` / `@page resize W H` — page dimensions in pixels.
    PageSize(f64, f64),
    With { graph: usize, set: Option<usize> },
    Target { graph: usize, set: usize },
    TypeDecl(SetType),
    GraphOnOff { graph: usize, on: bool },
    GraphHidden { graph: usize, hidden: bool },
    GraphType { graph: usize, ty: GraphType },
    GraphBargap { graph: usize, gap: f64 },
    GraphStacked { graph: usize, on: bool },
    World(WorldSpec),
    View(ViewSpec),
    Znorm(f64),
    Default(DefaultProp),
    Axis { axis: AxisId, prop: AxisProp },
    AxesScale { x: bool, scale: ScaleType },
    AxesInvert { x: bool, on: bool },
    Set { set: usize, prop: SetProp },
    Frame(FrameProp),
    Title(TextProp),
    Subtitle(TextProp),
    Legend(LegendProp),
    MapColor { index: i32, rgb: (u8, u8, u8) },
    /// `@map font N to \"Name\", \"Fallback\"`.
    MapFont { slot: usize, name: String },
    /// A recognized-but-ignored or unrecognized command.
    Unknown,
}

peg::parser! {
    /// Grammar over a single de-`@`-ed command line.
    pub grammar agr() for str {
        rule _() = quiet!{ [' ' | '\t']* }
        rule __() = quiet!{ [' ' | '\t']+ }
        rule comma() = _ "," _

        /// A run of ASCII letters compared case-insensitively to `k`.
        rule kw(k: &'static str) = w:$(['a'..='z' | 'A'..='Z']+) {?
            if w.eq_ignore_ascii_case(k) { Ok(()) } else { Err(k) }
        }

        rule digits() -> &'input str = $(['0'..='9']+)

        rule uint() -> usize = n:digits() {? n.parse().or(Err("uint")) }

        rule int() -> i32 = n:$("-"? ['0'..='9']+) {? n.parse().or(Err("int")) }

        rule num() -> f64 = n:$("-"? ['0'..='9']* ("." ['0'..='9']*)? (['e' | 'E'] ['+' | '-']? ['0'..='9']+)?) {?
            n.trim().parse::<f64>().or(Err("num"))
        }

        /// An integer that may be written with a fractional part (e.g. "1.000000").
        rule iflex() -> i32 = v:num() { v.round() as i32 }

        rule onoff() -> bool
            = kw("on") { true }
            / kw("true") { true }
            / kw("off") { false }
            / kw("false") { false }
            // Bare 0/1 must not swallow the start of a number ("legend 0.48,
            // 0.80"): PEG choice commits, so a prefix match here would make
            // the whole line unparsable.
            / "1" !['0'..='9' | '.'] { true }
            / "0" !['0'..='9' | '.'] { false }

        /// Double-quoted string. `\"` is an escaped quote (Grace's writer
        /// emits it); all other backslash sequences (text markup like `\n`,
        /// `\s`) are kept verbatim for the markup parser.
        rule qstring() -> String
            = "\"" s:$(("\\\"" / [^ '"'])*) "\"" { s.replace("\\\"", "\"") }
        rule word() -> &'input str = $(['a'..='z' | 'A'..='Z']+)

        /// `g0` / `G3` graph selector -> index.
        rule gsel() -> usize = ['g' | 'G'] n:uint() { n }
        /// `s0` / `S2` set selector -> index.
        rule ssel() -> usize = ['s' | 'S'] n:uint() { n }

        /// Entry point: parse a whole command line.
        pub rule command() -> Command
            = _ c:cmd() _ ![_] { c }
            / _ [_]* { Command::Unknown }

        rule cmd() -> Command
            = version()
            / page_cmd()
            / with()
            / object_cmd()
            / target()
            / type_decl()
            / graph_prefixed()
            / world_cmd()
            / view_cmd()
            / znorm_cmd()
            / default_cmd()
            / axes_cmd()
            / axis_cmd()
            / set_cmd()
            / frame_cmd()
            / title_cmd()
            / subtitle_cmd()
            / legend_cmd()
            / map_color()

        rule version() -> Command = kw("version") __ n:int() { Command::Version(n) }

        /// `@page size 600 600` / `@page resize 800, 600`. Other `@page ...`
        /// forms (scroll, inout, background) fall through to Unknown.
        rule page_cmd() -> Command
            = kw("page") __ (kw("size") / kw("resize")) __ w:num() (comma() / __) h:num() {
                Command::PageSize(w, h)
            }

        rule with() -> Command
            = kw("with") __ g:gsel() s:("." ['s'|'S'] n:uint() { n })? {
                Command::With { graph: g, set: s }
            }
            / kw("with") __ k:objkind() { Command::WithObject(k) }

        rule objkind() -> ObjKind
            = kw("string") { ObjKind::String }
            / kw("line") { ObjKind::Line }
            / kw("box") { ObjKind::Box }
            / kw("ellipse") { ObjKind::Ellipse }

        /// Property line of an open object definition: `@ string on`,
        /// `@ line 0.1, 0.2, 0.3, 0.4`, `@string def "text"`, `@box def` …
        rule object_cmd() -> Command
            = k:objkind() __ p:obj_prop() { Command::Object { kind: k, prop: p } }
            / kw("timestamp") __ p:obj_prop() { Command::Timestamp(p) }
            / kw("timestamp") { Command::Timestamp(ObjProp::Ignored) }

        rule obj_prop() -> ObjProp
            = kw("on") { ObjProp::On(true) }
            / kw("off") { ObjProp::On(false) }
            / kw("loctype") __ v:(kw("view") { true } / kw("world") { false }) {
                ObjProp::LoctypeView(v)
            }
            / kw("def") __ s:qstring() { ObjProp::Def(s) }
            / kw("def") { ObjProp::EndDef }
            / kw("color") __ n:iflex() { ObjProp::Color(n) }
            / kw("linewidth") __ v:num() { ObjProp::Linewidth(v) }
            / kw("linestyle") __ n:iflex() { ObjProp::Linestyle(n) }
            / kw("rot") __ v:num() { ObjProp::Rot(v) }
            / kw("font") __ n:iflex() { ObjProp::Font(n) }
            / kw("just") __ n:iflex() { ObjProp::Just(n) }
            / kw("char") __ kw("size") __ v:num() { ObjProp::CharSize(v) }
            / kw("fill") __ kw("color") __ n:iflex() { ObjProp::FillColor(n) }
            / kw("fill") __ kw("pattern") __ n:iflex() { ObjProp::FillPattern(n) }
            / kw("arrow") __ kw("type") __ n:iflex() { ObjProp::ArrowType(n) }
            / kw("arrow") __ (kw("length") / kw("size")) __ v:num() { ObjProp::ArrowLength(v) }
            / kw("arrow") __ kw("layout") __ d:num() comma() l:num() { ObjProp::ArrowLayout(d, l) }
            / kw("arrow") __ n:iflex() { ObjProp::ArrowEnd(n) }
            / g:gsel() { ObjProp::Graph(g) }
            / x1:num() comma() y1:num() comma() x2:num() comma() y2:num() {
                ObjProp::Pos4(x1, y1, x2, y2)
            }
            / x:num() comma() y:num() { ObjProp::Pos2(x, y) }
            / [_]* { ObjProp::Ignored }

        rule target() -> Command
            = kw("target") __ g:gsel() "." s:ssel() {
                Command::Target { graph: g, set: s }
            }

        rule type_decl() -> Command
            = kw("type") __ w:word() {
                match SetType::parse(w) {
                    Some(t) => Command::TypeDecl(t),
                    None => Command::Unknown,
                }
            }

        /// `g0 on`, `g0 hidden false`, `g0 type xy`.
        rule graph_prefixed() -> Command
            = g:gsel() __ p:graph_prop(g) { p }

        rule graph_prop(g: usize) -> Command
            = b:onoff() { Command::GraphOnOff { graph: g, on: b } }
            / kw("hidden") __ b:onoff() { Command::GraphHidden { graph: g, hidden: b } }
            / kw("type") __ w:word() {
                Command::GraphType { graph: g, ty: parse_graph_type(w) }
            }
            / kw("bar") __ kw("hgap") __ n:num() { Command::GraphBargap { graph: g, gap: n } }
            / kw("stacked") __ b:onoff() { Command::GraphStacked { graph: g, on: b } }
            / [_]* { Command::Unknown }

        rule world_cmd() -> Command
            = kw("world") __ s:world_spec() { Command::World(s) }
        rule world_spec() -> WorldSpec
            = b:bound() __ v:num() { WorldSpec::Component(b, v) }
            / a:num() comma() b:num() comma() c:num() comma() d:num() { WorldSpec::Full(a, b, c, d) }

        rule znorm_cmd() -> Command = kw("znorm") __ n:num() { Command::Znorm(n) }

        rule view_cmd() -> Command
            = kw("view") __ s:view_spec() { Command::View(s) }
        rule view_spec() -> ViewSpec
            = b:bound() __ v:num() { ViewSpec::Component(b, v) }
            / a:num() comma() b:num() comma() c:num() comma() d:num() { ViewSpec::Full(a, b, c, d) }

        rule bound() -> Bound
            = kw("xmin") { Bound::Xmin }
            / kw("xmax") { Bound::Xmax }
            / kw("ymin") { Bound::Ymin }
            / kw("ymax") { Bound::Ymax }

        rule default_cmd() -> Command
            = kw("default") __ p:default_prop() { Command::Default(p) }
        rule default_prop() -> DefaultProp
            = kw("linestyle") __ n:iflex() { DefaultProp::Linestyle(n) }
            / kw("linewidth") __ n:num() { DefaultProp::Linewidth(n) }
            / kw("color") __ n:iflex() { DefaultProp::Color(n) }
            / kw("pattern") __ n:iflex() { DefaultProp::Pattern(n) }
            / kw("char") __ kw("size") __ n:num() { DefaultProp::CharSize(n) }
            / kw("font") __ kw("source") __ [_]* { DefaultProp::Font(-1) } // ignored sentinel
            / kw("font") __ n:iflex() { DefaultProp::Font(n) }
            / kw("symbol") __ kw("size") __ n:num() { DefaultProp::SymbolSize(n) }
            / kw("sformat") __ [_]* { DefaultProp::Font(-1) }

        /// `xaxes scale Logarithmic`, `yaxes invert on`.
        rule axes_cmd() -> Command
            = x:axes_sel() __ kw("scale") __ w:word() {
                Command::AxesScale { x, scale: parse_scale(w) }
            }
            / x:axes_sel() __ kw("invert") __ b:onoff() {
                Command::AxesInvert { x, on: b }
            }
        rule axes_sel() -> bool
            = kw("xaxes") { true }
            / kw("yaxes") { false }

        rule axis_cmd() -> Command
            = a:axis_sel() __ p:axis_prop() { Command::Axis { axis: a, prop: p } }
        rule axis_sel() -> AxisId
            = kw("altxaxis") { AxisId::AltX }
            / kw("altyaxis") { AxisId::AltY }
            // Old xmgr names: the zero axes are the same slots as alt axes
            // (pars.yacc maps ZEROXAXIS to the ALTXAXIS token).
            / kw("zeroxaxis") { AxisId::AltX }
            / kw("zeroyaxis") { AxisId::AltY }
            / kw("xaxis") { AxisId::X }
            / kw("yaxis") { AxisId::Y }

        rule axis_prop() -> AxisProp
            = b:onoff() { AxisProp::Active(b) }
            / kw("bar") __ p:axis_bar() { p }
            / kw("label") __ p:axis_label() { p }
            / kw("tick") __ p:axis_tick() { p }
            / kw("ticklabel") __ p:axis_ticklabel() { p }
            / kw("type") __ kw("zero") __ b:onoff() { AxisProp::Zero(b) }
            / kw("offset") __ a:num() comma() b:num() { AxisProp::Offset(a, b) }
            / [_]* { AxisProp::Ignored }

        rule axis_bar() -> AxisProp
            = b:onoff() { AxisProp::BarActive(b) }
            / kw("color") __ n:iflex() { AxisProp::BarColor(n) }
            / kw("linestyle") __ n:iflex() { AxisProp::BarLinestyle(n) }
            / kw("linewidth") __ n:num() { AxisProp::BarLinewidth(n) }
            / [_]* { AxisProp::Ignored }

        rule axis_label() -> AxisProp
            = kw("char") __ kw("size") __ n:num() { AxisProp::LabelCharSize(n) }
            / kw("font") __ n:iflex() { AxisProp::LabelFont(n) }
            / kw("color") __ n:iflex() { AxisProp::LabelColor(n) }
            / kw("op") __ v:op_side() { AxisProp::LabelOp(v) }
            / kw("layout") __ v:(kw("perp") { true } / kw("para") { false }) {
                AxisProp::LabelPerp(v)
            }
            // "label place top|bottom|both|..." is the placement side too;
            // "label place auto|spec|normal" selects offset handling (auto
            // is our behavior, spec offsets are not modeled).
            / kw("place") __ v:op_side() { AxisProp::LabelOp(v) }
            / kw("place") __ (kw("auto") / kw("spec")) { AxisProp::Ignored }
            / s:qstring() { AxisProp::LabelText(s) }
            / [_]* { AxisProp::Ignored }

        rule axis_tick() -> AxisProp
            = b:onoff() { AxisProp::TicksActive(b) }
            / kw("in") { AxisProp::TicksInOut(0) }
            / kw("out") { AxisProp::TicksInOut(1) }
            / kw("both") { AxisProp::TicksInOut(2) }
            / kw("spec") __ kw("type") __ t:(
                  kw("none") { 0 } / kw("ticks") { 1 } / kw("both") { 2 }
              ) { AxisProp::SpecType(t) }
            / kw("spec") __ n:uint() { AxisProp::SpecCount(n) }
            / kw("type") __ kw("spec") { AxisProp::SpecMarksOld }
            / kw("type") __ kw("auto") { AxisProp::SpecType(0) }
            / kw("major") __ i:uint() comma() v:num() { AxisProp::SpecPos { idx: i, pos: v, major: true } }
            / kw("minor") __ i:uint() comma() v:num() { AxisProp::SpecPos { idx: i, pos: v, major: false } }
            / kw("major") __ p:tick_level(true) { AxisProp::Major(p) }
            / kw("minor") __ kw("ticks") __ n:iflex() { AxisProp::MinorTicks(n) }
            / kw("minor") __ p:tick_level(false) { AxisProp::Minor(p) }
            / kw("default") __ n:iflex() { AxisProp::AutoNum(n) }
            / kw("place") __ kw("rounded") __ b:onoff() { AxisProp::TickRound(b) }
            / kw("op") __ v:(
                  kw("both") { 2 }
                  / kw("bottom") { 0 } / kw("left") { 0 } / kw("normal") { 0 }
                  / kw("top") { 1 } / kw("right") { 1 } / kw("opposite") { 1 }
              ) { AxisProp::TickOp(v) }
            / [_]* { AxisProp::Ignored }

        /// Major (`spacing` allowed) / minor tick level properties.
        rule tick_level(allow_spacing: bool) -> TickLevelProp
            = kw("size") __ n:num() { TickLevelProp::Size(n) }
            / kw("color") __ n:iflex() { TickLevelProp::Color(n) }
            / kw("linewidth") __ n:num() { TickLevelProp::Linewidth(n) }
            / kw("linestyle") __ n:iflex() { TickLevelProp::Linestyle(n) }
            / kw("grid") __ b:onoff() { TickLevelProp::Grid(b) }
            / n:num() {? if allow_spacing { Ok(TickLevelProp::Spacing(n)) } else { Err("minor spacing") } }

        /// Placement side: 0 normal (bottom/left), 1 opposite (top/right), 2 both.
        rule op_side() -> i32
            = kw("both") { 2 }
            / kw("bottom") { 0 } / kw("left") { 0 } / kw("normal") { 0 }
            / kw("top") { 1 } / kw("right") { 1 } / kw("opposite") { 1 }

        rule axis_ticklabel() -> AxisProp
            // The spec form must precede onoff(): a bare "0" would otherwise
            // commit the PEG choice on "ticklabel 0, \"label\"" lines.
            = i:uint() comma() s:qstring() { AxisProp::SpecLabel { idx: i, label: s } }
            / b:onoff() { AxisProp::TlActive(b) }
            / kw("op") __ v:op_side() { AxisProp::TlOp(v) }
            / kw("type") __ kw("spec") { AxisProp::SpecType(2) }
            / kw("type") __ kw("auto") { AxisProp::SpecLabelsAutoOld }
            / kw("prec") __ n:iflex() { AxisProp::TlPrec(n) }
            / kw("skip") __ n:iflex() { AxisProp::TlSkip(n) }
            / kw("stagger") __ n:iflex() { AxisProp::TlStagger(n) }
            / kw("formula") __ s:qstring() { AxisProp::TlFormula(s) }
            / kw("format") __ w:word() {
                AxisProp::TlFormat(TickFormat::parse(w).unwrap_or(TickFormat::Decimal))
            }
            / kw("char") __ kw("size") __ n:num() { AxisProp::TlCharSize(n) }
            / kw("font") __ n:iflex() { AxisProp::TlFont(n) }
            / kw("color") __ n:iflex() { AxisProp::TlColor(n) }
            / kw("angle") __ n:iflex() { AxisProp::TlAngle(n) }
            / kw("append") __ s:qstring() { AxisProp::TlAppend(s) }
            / kw("prepend") __ s:qstring() { AxisProp::TlPrepend(s) }
            / kw("start") __ kw("type") __ w:word() { AxisProp::TlStartSpec(w.eq_ignore_ascii_case("spec")) }
            / kw("start") __ n:num() { AxisProp::TlStart(n) }
            / kw("stop") __ kw("type") __ w:word() { AxisProp::TlStopSpec(w.eq_ignore_ascii_case("spec")) }
            / kw("stop") __ n:num() { AxisProp::TlStop(n) }
            / [_]* { AxisProp::Ignored }

        rule set_cmd() -> Command
            = s:ssel() __ p:set_prop() { Command::Set { set: s, prop: p } }
        rule set_prop() -> SetProp
            = kw("type") __ w:word() {
                SetType::parse(w).map(SetProp::Type).unwrap_or(SetProp::Ignored)
            }
            / kw("hidden") __ b:onoff() { SetProp::Hidden(b) }
            / kw("symbol") __ p:set_symbol() { p }
            / kw("line") __ p:set_line() { p }
            / kw("fill") __ p:set_fill() { p }
            / kw("avalue") __ p:set_avalue() { p }
            / kw("errorbar") __ p:set_errorbar() { p }
            / kw("baseline") __ kw("type") __ n:iflex() { SetProp::BaselineType(n) }
            / kw("dropline") __ b:onoff() { SetProp::Dropline(b) }
            / kw("legend") __ s:qstring() { SetProp::Legend(s) }
            / kw("comment") __ s:qstring() { SetProp::Comment(s) }
            / kw("color") __ n:iflex() { SetProp::Color(n) }
            / kw("linewidth") __ n:num() { SetProp::Linewidth(n) }
            / kw("linestyle") __ n:iflex() { SetProp::Linestyle(n) }
            / [_]* { SetProp::Ignored }

        rule set_symbol() -> SetProp
            = kw("size") __ n:num() { SetProp::SymbolSize(n) }
            / kw("color") __ n:iflex() { SetProp::SymbolColor(n) }
            / kw("fill") __ kw("color") __ n:iflex() { SetProp::SymbolFillColor(n) }
            / kw("fill") __ kw("pattern") __ n:iflex() { SetProp::SymbolFillPattern(n) }
            / kw("char") __ kw("font") __ n:iflex() { SetProp::SymbolCharFont(n) }
            / kw("skip") __ n:iflex() { SetProp::SymbolSkip(n) }
            / kw("char") __ n:iflex() { SetProp::SymbolChar(n) }
            / kw("linewidth") __ n:num() { SetProp::SymbolLinewidth(n) }
            / kw("linestyle") __ n:iflex() { SetProp::SymbolLinestyle(n) }
            / n:iflex() { SetProp::Symbol(n) }
            / [_]* { SetProp::Ignored }

        rule set_line() -> SetProp
            = kw("type") __ n:iflex() { SetProp::LineType(n) }
            / kw("color") __ n:iflex() { SetProp::LineColor(n) }
            / kw("linewidth") __ n:num() { SetProp::LineLinewidth(n) }
            / kw("linestyle") __ n:iflex() { SetProp::LineLinestyle(n) }
            / [_]* { SetProp::Ignored }

        rule set_avalue() -> SetProp
            = b:onoff() { SetProp::AvOn(b) }
            / kw("type") __ n:iflex() { SetProp::AvType(n) }
            / kw("char") __ kw("size") __ v:num() { SetProp::AvSize(v) }
            / kw("font") __ n:iflex() { SetProp::AvFont(n) }
            / kw("color") __ n:iflex() { SetProp::AvColor(n) }
            / kw("rot") __ v:num() { SetProp::AvRot(v) }
            / kw("format") __ w:word() {
                SetProp::AvFormat(TickFormat::parse(w).unwrap_or(TickFormat::General))
            }
            / kw("prec") __ n:iflex() { SetProp::AvPrec(n) }
            / kw("offset") __ x:num() comma() y:num() { SetProp::AvOffset(x, y) }
            / kw("prepend") __ s:qstring() { SetProp::AvPrepend(s) }
            / kw("append") __ s:qstring() { SetProp::AvAppend(s) }
            / [_]* { SetProp::Ignored }

        rule set_errorbar() -> SetProp
            = b:onoff() { SetProp::EbOn(b) }
            / kw("place") __ v:(
                  kw("normal") { 0 } / kw("opposite") { 1 } / kw("both") { 2 }
              ) { SetProp::EbPlace(v) }
            / kw("color") __ n:iflex() { SetProp::EbColor(n) }
            / kw("size") __ v:num() { SetProp::EbSize(v) }
            / kw("length") __ v:num() { SetProp::EbSize(v) } // old name
            / kw("linewidth") __ v:num() { SetProp::EbLinewidth(v) }
            / kw("linestyle") __ n:iflex() { SetProp::EbLinestyle(n) }
            / kw("riser") __ kw("linewidth") __ v:num() { SetProp::EbRiserLinewidth(v) }
            / kw("riser") __ kw("linestyle") __ n:iflex() { SetProp::EbRiserLinestyle(n) }
            / kw("riser") __ kw("clip") __ kw("length") __ v:num() { SetProp::EbRiserClipLen(v) }
            / kw("riser") __ kw("clip") __ b:onoff() { SetProp::EbRiserClip(b) }
            / [_]* { SetProp::Ignored }

        rule set_fill() -> SetProp
            = kw("type") __ n:iflex() { SetProp::FillType(n) }
            / kw("rule") __ n:iflex() { SetProp::FillRule(n) }
            / kw("color") __ n:iflex() { SetProp::FillColor(n) }
            / kw("pattern") __ n:iflex() { SetProp::FillPattern(n) }
            / n:iflex() { SetProp::FillType(n) } // legacy "fill 1"
            / [_]* { SetProp::Ignored }

        rule frame_cmd() -> Command
            = kw("frame") __ p:frame_prop() { Command::Frame(p) }
        rule frame_prop() -> FrameProp
            = kw("type") __ n:iflex() { FrameProp::Type(n) }
            / kw("linestyle") __ n:iflex() { FrameProp::Linestyle(n) }
            / kw("linewidth") __ n:num() { FrameProp::Linewidth(n) }
            / kw("color") __ n:iflex() { FrameProp::Color(n) }
            / kw("pattern") __ n:iflex() { FrameProp::Pattern(n) }
            / kw("background") __ kw("color") __ n:iflex() { FrameProp::BackgroundColor(n) }
            / kw("background") __ kw("pattern") __ n:iflex() { FrameProp::BackgroundPattern(n) }

        rule title_cmd() -> Command
            = kw("title") __ p:text_prop() { Command::Title(p) }
        rule subtitle_cmd() -> Command
            = kw("subtitle") __ p:text_prop() { Command::Subtitle(p) }
        rule text_prop() -> TextProp
            = kw("font") __ n:iflex() { TextProp::Font(n) }
            / kw("size") __ n:num() { TextProp::Size(n) }
            / kw("color") __ n:iflex() { TextProp::Color(n) }
            / kw("linewidth") __ [_]* { TextProp::Color(-1) } // ignored
            / s:qstring() { TextProp::Text(s) }

        rule legend_cmd() -> Command
            = kw("legend") __ p:legend_prop() { Command::Legend(p) }
        rule legend_prop() -> LegendProp
            = b:onoff() { LegendProp::Active(b) }
            / kw("loctype") __ w:word() { LegendProp::LoctypeView(w.eq_ignore_ascii_case("view")) }
            / kw("font") __ n:iflex() { LegendProp::Font(n) }
            / kw("color") __ n:iflex() { LegendProp::Color(n) }
            / kw("char") __ kw("size") __ n:num() { LegendProp::CharSize(n) }
            / (['x' | 'X'] "1") __ n:num() { LegendProp::X(n) }
            / (['y' | 'Y'] "1") __ n:num() { LegendProp::Y(n) }
            / kw("length") __ n:num() { LegendProp::Length(n) }
            / kw("vgap") __ n:num() { LegendProp::Vgap(n) }
            / kw("hgap") __ n:num() { LegendProp::Hgap(n) }
            / kw("invert") __ b:onoff() { LegendProp::Invert(b) }
            / kw("box") __ p:legend_box() { p }
            / x:num() comma() y:num() { LegendProp::Position(x, y) }
            / [_]* { LegendProp::Ignored }

        rule legend_box() -> LegendProp
            = b:onoff() { LegendProp::BoxOn(b) }
            / kw("color") __ n:iflex() { LegendProp::BoxColor(n) }
            / kw("linewidth") __ n:num() { LegendProp::BoxLinewidth(n) }
            / kw("linestyle") __ n:iflex() { LegendProp::BoxLinestyle(n) }
            / kw("fill") __ kw("color") __ n:iflex() { LegendProp::BoxFillColor(n) }
            / kw("fill") __ kw("pattern") __ n:iflex() { LegendProp::BoxFillPattern(n) }
            / [_]* { LegendProp::Ignored }

        rule map_color() -> Command
            = kw("map") __ kw("color") __ idx:int() __ kw("to") __ "(" _ r:iflex() comma() g:iflex() comma() b:iflex() _ ")" rest:[_]* {
                Command::MapColor { index: idx, rgb: (clamp_u8(r), clamp_u8(g), clamp_u8(b)) }
            }
            / kw("map") __ kw("font") __ slot:uint() __ kw("to") _ s:qstring() rest:[_]* {
                Command::MapFont { slot, name: s }
            }
    }
}

fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

fn parse_graph_type(w: &str) -> GraphType {
    match w.to_ascii_lowercase().as_str() {
        "xy" => GraphType::Xy,
        "chart" => GraphType::Chart,
        "polar" => GraphType::Polar,
        "smith" => GraphType::Smith,
        "fixed" => GraphType::Fixed,
        "pie" => GraphType::Pie,
        _ => GraphType::Xy,
    }
}

fn parse_scale(w: &str) -> ScaleType {
    match w.to_ascii_lowercase().as_str() {
        "logarithmic" => ScaleType::Logarithmic,
        "reciprocal" => ScaleType::Reciprocal,
        "logit" => ScaleType::Logit,
        _ => ScaleType::Normal,
    }
}

/// Parse one de-`@`-ed command line, never failing (falls back to `Unknown`).
pub fn parse_line(line: &str) -> Command {
    agr::command(line).unwrap_or(Command::Unknown)
}
