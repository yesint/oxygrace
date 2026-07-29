//! Version 1 of the `.oxgr` schema: serde structs mirroring the model.
//!
//! The mirror is deliberate — the file format is decoupled from the
//! internal model, so internal refactors don't silently break files.
//! Every field name and enum variant name here is **file surface**;
//! rename with care (serde aliases keep old spellings readable).
//!
//! Writing omits everything equal to its baseline (fresh-model defaults),
//! so documents stay small and diff-friendly; reading fills omitted
//! fields from the same baselines, which makes omit-default lossless.
//! The corpus round-trip test (`tests/oxgr.rs`) holds this to
//! render-byte-equality across every example file.
//!
//! Magic integers are kept out of the file surface where Grace has
//! stable vocabulary: enums for dashes, placements, tick directions,
//! arrows and anchors. Color and font references stay palette/slot
//! indices (as in the model); text justification stays Grace's bit code.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::format::OxgrError;
use crate::model::{
    AValue, Axis, BoxObj, Defaults, ErrBar, FillType, Frame, Graph, GraphType, Legend, LineObj,
    LineType, Placement, Project, ScaleType, Set, StringObj, SymbolType, TickFormat, TickProps,
    World,
};

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    v == &T::default()
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// The baseline set all set documents diff against (fresh defaults).
fn baseline_set() -> Set {
    Set::with_defaults(&Defaults::default())
}

// ---------------------------------------------------------------- document

#[derive(Serialize, Deserialize, Debug)]
pub struct Document {
    /// Format revision; this module reads and writes revision 1.
    pub format: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub page: Page,
    #[serde(default, skip_serializing_if = "is_default")]
    pub defaults: DefaultsDoc,
    /// Font slot → embedded face indices; omitted = the modern Grace map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fonts: Option<Vec<i32>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<ColorDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphs: Vec<GraphDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strings: Vec<StringObjDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LineObjDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boxes: Vec<BoxObjDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ellipses: Vec<BoxObjDoc>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub timestamp: StringObjDoc,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct Page {
    pub width: u32,
    pub height: u32,
    #[serde(default = "dpi_default", skip_serializing_if = "is_dpi_default")]
    pub dpi: f64,
}

fn dpi_default() -> f64 {
    Project::default().dpi
}

fn is_dpi_default(v: &f64) -> bool {
    *v == dpi_default()
}

impl Default for Page {
    fn default() -> Self {
        let p = Project::default();
        Page { width: p.page_width, height: p.page_height, dpi: p.dpi }
    }
}

/// Project-wide defaults (`@default …`).
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct DefaultsDoc {
    pub color: i32,
    pub pattern: i32,
    pub dash: Dash,
    pub width: f64,
    pub char_size: f64,
    pub font: i32,
    pub symbol_size: f64,
}

impl Default for DefaultsDoc {
    fn default() -> Self {
        DefaultsDoc::from_model(&Defaults::default())
    }
}

impl DefaultsDoc {
    fn from_model(d: &Defaults) -> Self {
        DefaultsDoc {
            color: d.color,
            pattern: d.pattern,
            dash: Dash::from_code(d.linestyle),
            width: d.linewidth,
            char_size: d.charsize,
            font: d.font,
            symbol_size: d.symsize,
        }
    }

    fn to_model(&self) -> Defaults {
        Defaults {
            color: self.color,
            pattern: self.pattern,
            linestyle: self.dash.code(),
            linewidth: self.width,
            charsize: self.char_size,
            font: self.font,
            symsize: self.symbol_size,
        }
    }
}

/// One `@map color`-style override of the palette.
#[derive(Serialize, Deserialize, Debug)]
pub struct ColorDef {
    pub id: i32,
    /// `#rrggbb`.
    pub rgb: String,
}

// ------------------------------------------------------------------ graphs

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct GraphDoc {
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: GraphType,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// World window; always written (it is the plot's meaning).
    #[serde(default)]
    pub world: Span,
    /// Viewport in view coordinates; always written.
    #[serde(default = "view_default")]
    pub view: Span,
    #[serde(default, skip_serializing_if = "is_default")]
    pub xscale: ScaleType,
    #[serde(default, skip_serializing_if = "is_default")]
    pub yscale: ScaleType,
    #[serde(default, skip_serializing_if = "is_false")]
    pub xinvert: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub yinvert: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub stacked: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub bargap: f64,
    #[serde(default = "znorm_default", skip_serializing_if = "is_znorm_default")]
    pub znorm: f64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub title: Text,
    #[serde(default, skip_serializing_if = "is_default")]
    pub subtitle: Text,
    #[serde(default, skip_serializing_if = "is_default")]
    pub frame: FrameDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub legend: LegendDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub axes: Axes,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sets: Vec<SetDoc>,
}

fn znorm_default() -> f64 {
    1.0
}

fn is_znorm_default(v: &f64) -> bool {
    *v == 1.0
}

/// A rectangle span, used for both the world window and the viewport.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(default)]
pub struct Span {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

impl Default for Span {
    fn default() -> Self {
        let w = World::default();
        Span { xmin: w.xmin, xmax: w.xmax, ymin: w.ymin, ymax: w.ymax }
    }
}

fn view_default() -> Span {
    let g = Graph::default();
    Span {
        xmin: g.view.xmin,
        xmax: g.view.xmax,
        ymin: g.view.ymin,
        ymax: g.view.ymax,
    }
}

/// Title / subtitle text with its styling.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct Text {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    pub size: f64,
    pub font: i32,
    pub color: i32,
}

impl Default for Text {
    fn default() -> Self {
        // Baseline = the title slot of a fresh graph; the subtitle only
        // differs in size, which is always written.
        let l = crate::model::Labels::default();
        Text { text: String::new(), size: l.title_size, font: l.title_font, color: l.title_color }
    }
}

/// The frame around the plotting area.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct FrameDoc {
    /// Grace frame type (0 closed box, half-open variants 1..).
    pub kind: i32,
    pub color: i32,
    pub pattern: i32,
    pub dash: Dash,
    pub width: f64,
    pub fill: bool,
    pub fill_color: i32,
    pub fill_pattern: i32,
}

impl Default for FrameDoc {
    fn default() -> Self {
        FrameDoc::from_model(&Frame::default())
    }
}

impl FrameDoc {
    fn from_model(f: &Frame) -> Self {
        FrameDoc {
            kind: f.frame_type,
            color: f.pen.color,
            pattern: f.pen.pattern,
            dash: Dash::from_code(f.linestyle),
            width: f.linewidth,
            fill: f.fill,
            fill_color: f.fill_pen.color,
            fill_pattern: f.fill_pen.pattern,
        }
    }

    fn apply(&self, f: &mut Frame) {
        f.frame_type = self.kind;
        f.pen.color = self.color;
        f.pen.pattern = self.pattern;
        f.linestyle = self.dash.code();
        f.linewidth = self.width;
        f.fill = self.fill;
        f.fill_pen.color = self.fill_color;
        f.fill_pen.pattern = self.fill_pattern;
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct LegendDoc {
    pub show: bool,
    pub anchor: Anchor,
    pub x: f64,
    pub y: f64,
    pub font: i32,
    pub size: f64,
    pub color: i32,
    pub length: f64,
    pub vgap: f64,
    pub hgap: f64,
    pub invert: bool,
    pub box_on: bool,
    pub box_color: i32,
    pub box_width: f64,
    pub box_dash: Dash,
    pub box_fill_color: i32,
    pub box_fill_pattern: i32,
}

impl Default for LegendDoc {
    fn default() -> Self {
        LegendDoc::from_model(&Legend::default())
    }
}

impl LegendDoc {
    fn from_model(l: &Legend) -> Self {
        LegendDoc {
            show: l.active,
            anchor: Anchor::of(l.loctype_view),
            x: l.x,
            y: l.y,
            font: l.font,
            size: l.charsize,
            color: l.color,
            length: l.length,
            vgap: l.vgap,
            hgap: l.hgap,
            invert: l.invert,
            box_on: l.box_on,
            box_color: l.box_color,
            box_width: l.box_linewidth,
            box_dash: Dash::from_code(l.box_linestyle),
            box_fill_color: l.box_fill_color,
            box_fill_pattern: l.box_fill_pattern,
        }
    }

    fn apply(&self, l: &mut Legend) {
        l.active = self.show;
        l.loctype_view = self.anchor.is_view();
        l.x = self.x;
        l.y = self.y;
        l.font = self.font;
        l.charsize = self.size;
        l.color = self.color;
        l.length = self.length;
        l.vgap = self.vgap;
        l.hgap = self.hgap;
        l.invert = self.invert;
        l.box_on = self.box_on;
        l.box_color = self.box_color;
        l.box_linewidth = self.box_width;
        l.box_linestyle = self.box_dash.code();
        l.box_fill_color = self.box_fill_color;
        l.box_fill_pattern = self.box_fill_pattern;
    }
}

// -------------------------------------------------------------------- axes

/// The four per-graph axes. A `None` slot keeps that axis at the model's
/// own baseline (alt axes default to inactive, x/y to active).
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Axes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<AxisDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<AxisDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altx: Option<AxisDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alty: Option<AxisDoc>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct AxisDoc {
    pub active: bool,
    /// Axis sits at world zero of the perpendicular coordinate.
    #[serde(default, skip_serializing_if = "is_false")]
    pub zero: bool,
    /// Outward shifts of the normal / opposite positions, in view units.
    #[serde(default, skip_serializing_if = "is_default")]
    pub offset: (f64, f64),
    #[serde(default, skip_serializing_if = "is_default")]
    pub bar: AxisBarDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub label: AxisLabelDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub ticks: TicksDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tick_labels: TickLabelsDoc,
}

impl Default for AxisDoc {
    fn default() -> Self {
        AxisDoc::from_model(&Axis::default())
    }
}

/// The axis bar (the line along the frame edge).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct AxisBarDoc {
    pub on: bool,
    pub color: i32,
    pub dash: Dash,
    pub width: f64,
}

impl Default for AxisBarDoc {
    fn default() -> Self {
        let a = Axis::default();
        AxisBarDoc {
            on: a.draw_bar,
            color: a.bar_color,
            dash: Dash::from_code(a.bar_linestyle),
            width: a.bar_linewidth,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct AxisLabelDoc {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    pub side: Placement,
    /// Perpendicular label layout (`label layout perp`).
    pub perp: bool,
    pub size: f64,
    pub font: i32,
    pub color: i32,
}

impl Default for AxisLabelDoc {
    fn default() -> Self {
        let a = Axis::default();
        AxisLabelDoc {
            text: String::new(),
            side: placement_of(a.label_op),
            perp: a.label_perp,
            size: a.label_charsize,
            font: a.label_font,
            color: a.label_color,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct TicksDoc {
    pub on: bool,
    /// Major tick spacing in world units.
    pub major: f64,
    /// Minor intervals per major.
    pub minor: i32,
    /// Desired major tick count for autoticking.
    pub auto: i32,
    /// Round the first major down to a spacing multiple.
    pub round: bool,
    pub direction: TickDirection,
    /// Which frame side carries ticks.
    pub side: Placement,
    pub major_style: TickStyle,
    pub minor_style: TickStyle,
    /// Specified ticks: positions (and labels, for `SpecKind::Labels`)
    /// come from `spec` instead of the generator.
    #[serde(default, skip_serializing_if = "is_default")]
    pub spec_kind: SpecKind,
    #[serde(default, skip_serializing_if = "is_default")]
    pub spec_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spec: Vec<SpecTickDoc>,
}

impl Default for TicksDoc {
    fn default() -> Self {
        let a = Axis::default();
        TicksDoc {
            on: a.ticks,
            major: a.major,
            minor: a.minor_ticks,
            auto: a.autonum,
            round: a.tick_round,
            direction: TickDirection::from_code(a.tick_inout),
            side: placement_of(a.op),
            major_style: TickStyle::from_model(&a.major_props),
            minor_style: TickStyle::from_model(&a.minor_props),
            spec_kind: SpecKind::Generated,
            spec_count: 0,
            spec: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct TickStyle {
    pub size: f64,
    pub color: i32,
    pub width: f64,
    pub dash: Dash,
    pub grid: bool,
}

impl Default for TickStyle {
    fn default() -> Self {
        TickStyle::from_model(&TickProps::default())
    }
}

impl TickStyle {
    fn from_model(t: &TickProps) -> Self {
        TickStyle {
            size: t.size,
            color: t.color,
            width: t.linewidth,
            dash: Dash::from_code(t.linestyle),
            grid: t.grid,
        }
    }

    fn to_model(&self) -> TickProps {
        TickProps {
            size: self.size,
            color: self.color,
            linewidth: self.width,
            linestyle: self.dash.code(),
            grid: self.grid,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpecKind {
    #[default]
    Generated,
    Positions,
    Labels,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SpecTickDoc {
    pub pos: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub minor: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct TickLabelsDoc {
    pub on: bool,
    pub format: TickFormat,
    pub prec: i32,
    pub size: f64,
    pub font: i32,
    pub color: i32,
    pub angle: i32,
    /// Label every (skip+1)-th major tick.
    pub skip: i32,
    /// Alternate labels over N+1 rows.
    pub stagger: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prepend: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub append: String,
    /// `$t` arithmetic applied before formatting.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub formula: String,
    /// Which frame side carries the labels.
    pub side: Placement,
    /// Label only ticks at ≥ start / ≤ stop (`None` = unrestricted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<f64>,
}

impl Default for TickLabelsDoc {
    fn default() -> Self {
        let a = Axis::default();
        TickLabelsDoc {
            on: a.ticklabels,
            format: a.tl_format,
            prec: a.tl_prec,
            size: a.tl_charsize,
            font: a.tl_font,
            color: a.tl_color,
            angle: a.tl_angle,
            skip: a.tl_skip,
            stagger: a.tl_stagger,
            prepend: String::new(),
            append: String::new(),
            formula: String::new(),
            side: placement_of(a.tl_op),
            start: None,
            stop: None,
        }
    }
}

impl AxisDoc {
    fn from_model(a: &Axis) -> Self {
        AxisDoc {
            active: a.active,
            zero: a.zero,
            offset: (a.offs_normal, a.offs_opposite),
            bar: AxisBarDoc {
                on: a.draw_bar,
                color: a.bar_color,
                dash: Dash::from_code(a.bar_linestyle),
                width: a.bar_linewidth,
            },
            label: AxisLabelDoc {
                text: a.label.clone(),
                side: placement_of(a.label_op),
                perp: a.label_perp,
                size: a.label_charsize,
                font: a.label_font,
                color: a.label_color,
            },
            ticks: TicksDoc {
                on: a.ticks,
                major: a.major,
                minor: a.minor_ticks,
                auto: a.autonum,
                round: a.tick_round,
                direction: TickDirection::from_code(a.tick_inout),
                side: placement_of(a.op),
                major_style: TickStyle::from_model(&a.major_props),
                minor_style: TickStyle::from_model(&a.minor_props),
                spec_kind: match a.spec_type {
                    1 => SpecKind::Positions,
                    2 => SpecKind::Labels,
                    _ => SpecKind::Generated,
                },
                spec_count: a.spec_count,
                spec: a
                    .spec_ticks
                    .iter()
                    .map(|t| SpecTickDoc { pos: t.pos, minor: !t.major, label: t.label.clone() })
                    .collect(),
            },
            tick_labels: TickLabelsDoc {
                on: a.ticklabels,
                format: a.tl_format,
                prec: a.tl_prec,
                size: a.tl_charsize,
                font: a.tl_font,
                color: a.tl_color,
                angle: a.tl_angle,
                skip: a.tl_skip,
                stagger: a.tl_stagger,
                prepend: a.tl_prepend.clone(),
                append: a.tl_append.clone(),
                formula: a.tl_formula.clone(),
                side: placement_of(a.tl_op),
                start: a.tl_start_spec.then_some(a.tl_start),
                stop: a.tl_stop_spec.then_some(a.tl_stop),
            },
        }
    }

    fn apply(&self, a: &mut Axis) {
        a.active = self.active;
        a.zero = self.zero;
        a.offs_normal = self.offset.0;
        a.offs_opposite = self.offset.1;
        a.draw_bar = self.bar.on;
        a.bar_color = self.bar.color;
        a.bar_linestyle = self.bar.dash.code();
        a.bar_linewidth = self.bar.width;
        a.label = self.label.text.clone();
        a.label_op = placement_code(self.label.side);
        a.label_perp = self.label.perp;
        a.label_charsize = self.label.size;
        a.label_font = self.label.font;
        a.label_color = self.label.color;
        a.ticks = self.ticks.on;
        a.major = self.ticks.major;
        a.minor_ticks = self.ticks.minor;
        a.autonum = self.ticks.auto;
        a.tick_round = self.ticks.round;
        a.tick_inout = self.ticks.direction.code();
        a.op = placement_code(self.ticks.side);
        a.major_props = self.ticks.major_style.to_model();
        a.minor_props = self.ticks.minor_style.to_model();
        a.spec_type = match self.ticks.spec_kind {
            SpecKind::Generated => 0,
            SpecKind::Positions => 1,
            SpecKind::Labels => 2,
        };
        a.spec_count = self.ticks.spec_count;
        a.spec_ticks = self
            .ticks
            .spec
            .iter()
            .map(|t| crate::model::SpecTick {
                pos: t.pos,
                major: !t.minor,
                label: t.label.clone(),
            })
            .collect();
        a.ticklabels = self.tick_labels.on;
        a.tl_format = self.tick_labels.format;
        a.tl_prec = self.tick_labels.prec;
        a.tl_charsize = self.tick_labels.size;
        a.tl_font = self.tick_labels.font;
        a.tl_color = self.tick_labels.color;
        a.tl_angle = self.tick_labels.angle;
        a.tl_skip = self.tick_labels.skip;
        a.tl_stagger = self.tick_labels.stagger;
        a.tl_prepend = self.tick_labels.prepend.clone();
        a.tl_append = self.tick_labels.append.clone();
        a.tl_formula = self.tick_labels.formula.clone();
        a.tl_op = placement_code(self.tick_labels.side);
        a.tl_start_spec = self.tick_labels.start.is_some();
        a.tl_start = self.tick_labels.start.unwrap_or(0.0);
        a.tl_stop_spec = self.tick_labels.stop.is_some();
        a.tl_stop = self.tick_labels.stop.unwrap_or(0.0);
    }
}

fn placement_of(op: i32) -> Placement {
    match op {
        1 => Placement::Opposite,
        2 => Placement::Both,
        _ => Placement::Normal,
    }
}

fn placement_code(p: Placement) -> i32 {
    match p {
        Placement::Normal => 0,
        Placement::Opposite => 1,
        Placement::Both => 2,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickDirection {
    #[default]
    In,
    Out,
    Both,
}

impl TickDirection {
    fn from_code(code: i32) -> Self {
        match code {
            1 => TickDirection::Out,
            2 => TickDirection::Both,
            _ => TickDirection::In,
        }
    }

    fn code(self) -> i32 {
        match self {
            TickDirection::In => 0,
            TickDirection::Out => 1,
            TickDirection::Both => 2,
        }
    }
}

/// View- vs world-anchored coordinates (objects, legend).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    View,
    World,
}

impl Anchor {
    fn of(loctype_view: bool) -> Self {
        if loctype_view { Anchor::View } else { Anchor::World }
    }

    fn is_view(self) -> bool {
        self == Anchor::View
    }
}

// -------------------------------------------------------------------- sets

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct SetDoc {
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: crate::model::SetType,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub legend: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub line: LineDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub symbol: SymbolDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub fill: FillDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub error_bars: ErrBarDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub value_labels: AValueDoc,
    /// Inline data: whitespace-separated columns, one point per row, an
    /// optional trailing quoted string per point — the `.agr` data block
    /// as a raw string.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data: String,
}

/// The connecting line (Grace line pen).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct LineDoc {
    pub kind: LineType,
    pub dash: Dash,
    pub width: f64,
    pub color: i32,
    /// 0..=255 (QtGrace pen alpha).
    pub opacity: i32,
    /// Vertical drop lines from each point to the baseline.
    #[serde(default, skip_serializing_if = "is_false")]
    pub droplines: bool,
}

impl Default for LineDoc {
    fn default() -> Self {
        LineDoc::from_model(&baseline_set())
    }
}

impl LineDoc {
    fn from_model(s: &Set) -> Self {
        LineDoc {
            kind: s.line_type,
            dash: Dash::from_code(s.linestyle),
            width: s.linewidth,
            color: s.line_pen.color,
            opacity: s.line_pen.alpha,
            droplines: s.dropline,
        }
    }

    fn apply(&self, s: &mut Set) {
        s.line_type = self.kind;
        s.linestyle = self.dash.code();
        s.linewidth = self.width;
        s.line_pen.color = self.color;
        s.line_pen.alpha = self.opacity;
        s.dropline = self.droplines;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct SymbolDoc {
    pub shape: SymbolType,
    pub size: f64,
    pub color: i32,
    pub opacity: i32,
    pub fill_color: i32,
    /// Grace fill pattern index (0 none, 1 solid, 2..=31 hatches).
    pub fill_pattern: i32,
    pub fill_opacity: i32,
    pub width: f64,
    pub dash: Dash,
    /// Draw every (skip+1)-th symbol.
    pub skip: i32,
    /// Character code + font slot for `Char` symbols.
    #[serde(default, skip_serializing_if = "is_default")]
    pub char: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub char_font: i32,
}

impl Default for SymbolDoc {
    fn default() -> Self {
        SymbolDoc::from_model(&baseline_set())
    }
}

impl SymbolDoc {
    fn from_model(s: &Set) -> Self {
        SymbolDoc {
            shape: s.symbol,
            size: s.symbol_size,
            color: s.symbol_pen.color,
            opacity: s.symbol_pen.alpha,
            fill_color: s.symbol_fill.color,
            fill_pattern: s.symbol_fill.pattern,
            fill_opacity: s.symbol_fill.alpha,
            width: s.symbol_linewidth,
            dash: Dash::from_code(s.symbol_linestyle),
            skip: s.symskip,
            char: s.symbol_char,
            char_font: s.symbol_char_font,
        }
    }

    fn apply(&self, s: &mut Set) {
        s.symbol = self.shape;
        s.symbol_size = self.size;
        s.symbol_pen.color = self.color;
        s.symbol_pen.alpha = self.opacity;
        s.symbol_fill.color = self.fill_color;
        s.symbol_fill.pattern = self.fill_pattern;
        s.symbol_fill.alpha = self.fill_opacity;
        s.symbol_linewidth = self.width;
        s.symbol_linestyle = self.dash.code();
        s.symskip = self.skip;
        s.symbol_char = self.char;
        s.symbol_char_font = self.char_font;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct FillDoc {
    pub kind: FillType,
    /// 0 winding, 1 even-odd.
    pub rule: i32,
    pub color: i32,
    pub pattern: i32,
    pub opacity: i32,
    /// Baseline type for baseline fills (Grace `setybase`).
    pub baseline: i32,
}

impl Default for FillDoc {
    fn default() -> Self {
        FillDoc::from_model(&baseline_set())
    }
}

impl FillDoc {
    fn from_model(s: &Set) -> Self {
        FillDoc {
            kind: s.fill_type,
            rule: s.fill_rule,
            color: s.fill_pen.color,
            pattern: s.fill_pen.pattern,
            opacity: s.fill_pen.alpha,
            baseline: s.baseline_type,
        }
    }

    fn apply(&self, s: &mut Set) {
        s.fill_type = self.kind;
        s.fill_rule = self.rule;
        s.fill_pen.color = self.color;
        s.fill_pen.pattern = self.pattern;
        s.fill_pen.alpha = self.opacity;
        s.baseline_type = self.baseline;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct ErrBarDoc {
    pub on: bool,
    pub side: Placement,
    pub color: i32,
    pub opacity: i32,
    /// Cap half-length factor.
    pub size: f64,
    pub width: f64,
    pub dash: Dash,
    pub riser_width: f64,
    pub riser_dash: Dash,
    /// Clip overlong risers at `clip_length` with an open arrow.
    pub arrow_clip: bool,
    pub clip_length: f64,
}

impl Default for ErrBarDoc {
    fn default() -> Self {
        ErrBarDoc::from_model(&ErrBar::default())
    }
}

impl ErrBarDoc {
    fn from_model(e: &ErrBar) -> Self {
        ErrBarDoc {
            on: e.active,
            side: placement_of(e.place),
            color: e.color,
            opacity: e.alpha,
            size: e.size,
            width: e.linewidth,
            dash: Dash::from_code(e.linestyle),
            riser_width: e.riser_linewidth,
            riser_dash: Dash::from_code(e.riser_linestyle),
            arrow_clip: e.arrow_clip,
            clip_length: e.cliplen,
        }
    }

    fn apply(&self, e: &mut ErrBar) {
        e.active = self.on;
        e.place = placement_code(self.side);
        e.color = self.color;
        e.alpha = self.opacity;
        e.size = self.size;
        e.linewidth = self.width;
        e.linestyle = self.dash.code();
        e.riser_linewidth = self.riser_width;
        e.riser_linestyle = self.riser_dash.code();
        e.arrow_clip = self.arrow_clip;
        e.cliplen = self.clip_length;
    }
}

/// Annotated point values (Grace avalue).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct AValueDoc {
    pub on: bool,
    /// 0 none, 1 X, 2 Y, 3 XY, 4 per-point string, 5 Z column.
    pub kind: i32,
    pub size: f64,
    pub font: i32,
    pub color: i32,
    pub opacity: i32,
    pub angle: f64,
    pub format: TickFormat,
    pub prec: i32,
    pub offset: (f64, f64),
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prepend: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub append: String,
}

impl Default for AValueDoc {
    fn default() -> Self {
        AValueDoc::from_model(&AValue::default())
    }
}

impl AValueDoc {
    fn from_model(a: &AValue) -> Self {
        AValueDoc {
            on: a.active,
            kind: a.avtype,
            size: a.size,
            font: a.font,
            color: a.color,
            opacity: a.alpha,
            angle: a.angle,
            format: a.format,
            prec: a.prec,
            offset: (a.offx, a.offy),
            prepend: a.prepend.clone(),
            append: a.append.clone(),
        }
    }

    fn apply(&self, a: &mut AValue) {
        a.active = self.on;
        a.avtype = self.kind;
        a.size = self.size;
        a.font = self.font;
        a.color = self.color;
        a.alpha = self.opacity;
        a.angle = self.angle;
        a.format = self.format;
        a.prec = self.prec;
        a.offx = self.offset.0;
        a.offy = self.offset.1;
        a.prepend = self.prepend.clone();
        a.append = self.append.clone();
    }
}

/// Grace's nine line styles by name instead of index. (`Off` instead of
/// `None`: RON would have to escape `None` as `r#None` under
/// `implicit_some` — same for the model's Symbol/Line/Fill `None`s.)
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dash {
    #[serde(rename = "Off")]
    None,
    #[default]
    Solid,
    Dotted,
    Dashed,
    LongDash,
    DotDash,
    DotLongDash,
    DotDashDot,
    DashDotDash,
}

impl Dash {
    fn from_code(code: i32) -> Self {
        use Dash::*;
        match code {
            0 => None,
            2 => Dotted,
            3 => Dashed,
            4 => LongDash,
            5 => DotDash,
            6 => DotLongDash,
            7 => DotDashDot,
            8 => DashDotDash,
            _ => Solid,
        }
    }

    fn code(self) -> i32 {
        use Dash::*;
        match self {
            None => 0,
            Solid => 1,
            Dotted => 2,
            Dashed => 3,
            LongDash => 4,
            DotDash => 5,
            DotLongDash => 6,
            DotDashDot => 7,
            DashDotDash => 8,
        }
    }
}

// ----------------------------------------------------------------- objects

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct StringObjDoc {
    pub active: bool,
    pub anchor: Anchor,
    /// Owning graph when world-anchored.
    #[serde(default, skip_serializing_if = "is_default")]
    pub graph: usize,
    pub x: f64,
    pub y: f64,
    pub color: i32,
    pub rot: f64,
    pub font: i32,
    /// Grace justification bits: h = just & 3 (0 left, 1 right, 2 center),
    /// v = just & 12 (0 baseline, 4 bottom, 8 top, 12 middle).
    pub just: i32,
    pub size: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
}

impl Default for StringObjDoc {
    fn default() -> Self {
        StringObjDoc::from_model(&StringObj::with_defaults(&Defaults::default()))
    }
}

impl StringObjDoc {
    fn from_model(s: &StringObj) -> Self {
        StringObjDoc {
            active: s.active,
            anchor: Anchor::of(s.loctype_view),
            graph: s.gno,
            x: s.x,
            y: s.y,
            color: s.color,
            rot: s.rot,
            font: s.font,
            just: s.just,
            size: s.charsize,
            text: s.text.clone(),
        }
    }

    fn into_model(self) -> StringObj {
        StringObj {
            active: self.active,
            loctype_view: self.anchor.is_view(),
            gno: self.graph,
            x: self.x,
            y: self.y,
            color: self.color,
            rot: self.rot,
            font: self.font,
            just: self.just,
            charsize: self.size,
            text: self.text,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct LineObjDoc {
    pub active: bool,
    pub anchor: Anchor,
    #[serde(default, skip_serializing_if = "is_default")]
    pub graph: usize,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub width: f64,
    pub dash: Dash,
    pub color: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub arrows: ArrowDoc,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct ArrowDoc {
    pub ends: ArrowEnds,
    pub kind: ArrowKind,
    /// Head length factor (view length = 0.01 × length).
    pub length: f64,
    /// Layout form factors d/L and l/L.
    pub dl: f64,
    pub ll: f64,
}

impl Default for ArrowDoc {
    fn default() -> Self {
        let l = LineObj::with_defaults(&Defaults::default());
        ArrowDoc {
            ends: ArrowEnds::from_code(l.arrow_end),
            kind: ArrowKind::from_code(l.arrow_type),
            length: l.arrow_length,
            dl: l.arrow_dl,
            ll: l.arrow_ll,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrowEnds {
    #[default]
    Off,
    Start,
    End,
    Both,
}

impl ArrowEnds {
    fn from_code(code: i32) -> Self {
        match code {
            1 => ArrowEnds::Start,
            2 => ArrowEnds::End,
            3 => ArrowEnds::Both,
            _ => ArrowEnds::Off,
        }
    }

    fn code(self) -> i32 {
        match self {
            ArrowEnds::Off => 0,
            ArrowEnds::Start => 1,
            ArrowEnds::End => 2,
            ArrowEnds::Both => 3,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrowKind {
    #[default]
    Open,
    Filled,
    BgFilled,
}

impl ArrowKind {
    fn from_code(code: i32) -> Self {
        match code {
            1 => ArrowKind::Filled,
            2 => ArrowKind::BgFilled,
            _ => ArrowKind::Open,
        }
    }

    fn code(self) -> i32 {
        match self {
            ArrowKind::Open => 0,
            ArrowKind::Filled => 1,
            ArrowKind::BgFilled => 2,
        }
    }
}

impl Default for LineObjDoc {
    fn default() -> Self {
        LineObjDoc::from_model(&LineObj::with_defaults(&Defaults::default()))
    }
}

impl LineObjDoc {
    fn from_model(l: &LineObj) -> Self {
        LineObjDoc {
            active: l.active,
            anchor: Anchor::of(l.loctype_view),
            graph: l.gno,
            x1: l.x1,
            y1: l.y1,
            x2: l.x2,
            y2: l.y2,
            width: l.linewidth,
            dash: Dash::from_code(l.linestyle),
            color: l.color,
            arrows: ArrowDoc {
                ends: ArrowEnds::from_code(l.arrow_end),
                kind: ArrowKind::from_code(l.arrow_type),
                length: l.arrow_length,
                dl: l.arrow_dl,
                ll: l.arrow_ll,
            },
        }
    }

    fn into_model(self) -> LineObj {
        LineObj {
            active: self.active,
            loctype_view: self.anchor.is_view(),
            gno: self.graph,
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
            linewidth: self.width,
            linestyle: self.dash.code(),
            color: self.color,
            arrow_end: self.arrows.ends.code(),
            arrow_type: self.arrows.kind.code(),
            arrow_length: self.arrows.length,
            arrow_dl: self.arrows.dl,
            arrow_ll: self.arrows.ll,
        }
    }
}

/// Boxes and ellipses share the model type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct BoxObjDoc {
    pub active: bool,
    pub anchor: Anchor,
    #[serde(default, skip_serializing_if = "is_default")]
    pub graph: usize,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub width: f64,
    pub dash: Dash,
    pub color: i32,
    pub fill_color: i32,
    pub fill_pattern: i32,
}

impl Default for BoxObjDoc {
    fn default() -> Self {
        BoxObjDoc::from_model(&BoxObj::with_defaults(&Defaults::default()))
    }
}

impl BoxObjDoc {
    fn from_model(b: &BoxObj) -> Self {
        BoxObjDoc {
            active: b.active,
            anchor: Anchor::of(b.loctype_view),
            graph: b.gno,
            x1: b.x1,
            y1: b.y1,
            x2: b.x2,
            y2: b.y2,
            width: b.linewidth,
            dash: Dash::from_code(b.linestyle),
            color: b.color,
            fill_color: b.fill_color,
            fill_pattern: b.fill_pattern,
        }
    }

    fn into_model(self) -> BoxObj {
        BoxObj {
            active: self.active,
            loctype_view: self.anchor.is_view(),
            gno: self.graph,
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
            linewidth: self.width,
            linestyle: self.dash.code(),
            color: self.color,
            fill_color: self.fill_color,
            fill_pattern: self.fill_pattern,
        }
    }
}

// ------------------------------------------------------------- conversions

impl Document {
    /// Build the v1 document mirror of a project.
    pub fn from_project(p: &Project) -> Document {
        Document {
            format: 1,
            page: Page { width: p.page_width, height: p.page_height, dpi: p.dpi },
            defaults: DefaultsDoc::from_model(&p.defaults),
            fonts: (p.font_map != crate::font::FONT_MAP_DEFAULT)
                .then(|| p.font_map.to_vec()),
            colors: p
                .color_overrides
                .iter()
                // .agr files habitually re-declare the stock palette; only
                // real overrides are worth keeping.
                .filter(|&&(id, rgb)| {
                    usize::try_from(id)
                        .ok()
                        .and_then(|i| crate::color::DEFAULT_COLORMAP.get(i))
                        != Some(&rgb)
                })
                .map(|&(id, (r, g, b))| ColorDef {
                    id,
                    rgb: format!("#{r:02x}{g:02x}{b:02x}"),
                })
                .collect(),
            graphs: p.graphs.iter().map(GraphDoc::from_model).collect(),
            strings: p.strings.iter().map(StringObjDoc::from_model).collect(),
            lines: p.lines.iter().map(LineObjDoc::from_model).collect(),
            boxes: p.boxes.iter().map(BoxObjDoc::from_model).collect(),
            ellipses: p.ellipses.iter().map(BoxObjDoc::from_model).collect(),
            timestamp: StringObjDoc::from_model(&p.timestamp),
        }
    }

    /// Materialize a project from the document.
    pub fn into_project(self) -> Result<Project, OxgrError> {
        let mut p = Project {
            page_width: self.page.width,
            page_height: self.page.height,
            dpi: self.page.dpi,
            defaults: self.defaults.to_model(),
            ..Default::default()
        };
        if let Some(fonts) = &self.fonts {
            for (slot, &face) in p.font_map.iter_mut().zip(fonts) {
                *slot = face;
            }
        }
        for c in &self.colors {
            let rgb = parse_hex(&c.rgb)
                .ok_or_else(|| OxgrError::Color { id: c.id, rgb: c.rgb.clone() })?;
            p.color_overrides.push((c.id, rgb));
        }
        let defaults = p.defaults;
        p.graphs = self.graphs.into_iter().map(|g| g.into_model(&defaults)).collect();
        p.strings = self.strings.into_iter().map(StringObjDoc::into_model).collect();
        p.lines = self.lines.into_iter().map(LineObjDoc::into_model).collect();
        p.boxes = self.boxes.into_iter().map(BoxObjDoc::into_model).collect();
        p.ellipses = self.ellipses.into_iter().map(BoxObjDoc::into_model).collect();
        p.timestamp = self.timestamp.into_model();
        Ok(p)
    }
}

impl GraphDoc {
    fn from_model(g: &Graph) -> GraphDoc {
        let base = Graph::default();
        let axis_slot = |i: usize| -> Option<AxisDoc> {
            let doc = AxisDoc::from_model(&g.axes[i]);
            // Emit only axes that differ from that slot's own baseline
            // (alt axes default to inactive, x/y to active).
            (doc != AxisDoc::from_model(&base.axes[i])).then_some(doc)
        };
        GraphDoc {
            kind: g.graph_type,
            hidden: g.hidden,
            world: Span {
                xmin: g.world.xmin,
                xmax: g.world.xmax,
                ymin: g.world.ymin,
                ymax: g.world.ymax,
            },
            view: Span {
                xmin: g.view.xmin,
                xmax: g.view.xmax,
                ymin: g.view.ymin,
                ymax: g.view.ymax,
            },
            xscale: g.xscale,
            yscale: g.yscale,
            xinvert: g.xinvert,
            yinvert: g.yinvert,
            stacked: g.stacked,
            bargap: g.bargap,
            znorm: g.znorm,
            title: Text {
                text: g.labels.title.clone(),
                size: g.labels.title_size,
                font: g.labels.title_font,
                color: g.labels.title_color,
            },
            subtitle: Text {
                text: g.labels.subtitle.clone(),
                size: g.labels.subtitle_size,
                font: g.labels.subtitle_font,
                color: g.labels.subtitle_color,
            },
            frame: FrameDoc::from_model(&g.frame),
            legend: LegendDoc::from_model(&g.legend),
            axes: Axes {
                x: axis_slot(0),
                y: axis_slot(1),
                altx: axis_slot(2),
                alty: axis_slot(3),
            },
            sets: g.sets.iter().map(SetDoc::from_model).collect(),
        }
    }

    fn into_model(self, defaults: &Defaults) -> Graph {
        let mut g = Graph {
            graph_type: self.kind,
            hidden: self.hidden,
            world: World {
                xmin: self.world.xmin,
                xmax: self.world.xmax,
                ymin: self.world.ymin,
                ymax: self.world.ymax,
            },
            ..Default::default()
        };
        g.view.xmin = self.view.xmin;
        g.view.xmax = self.view.xmax;
        g.view.ymin = self.view.ymin;
        g.view.ymax = self.view.ymax;
        g.xscale = self.xscale;
        g.yscale = self.yscale;
        g.xinvert = self.xinvert;
        g.yinvert = self.yinvert;
        g.stacked = self.stacked;
        g.bargap = self.bargap;
        g.znorm = self.znorm;
        g.labels.title = self.title.text;
        g.labels.title_size = self.title.size;
        g.labels.title_font = self.title.font;
        g.labels.title_color = self.title.color;
        g.labels.subtitle = self.subtitle.text;
        g.labels.subtitle_size = self.subtitle.size;
        g.labels.subtitle_font = self.subtitle.font;
        g.labels.subtitle_color = self.subtitle.color;
        self.frame.apply(&mut g.frame);
        self.legend.apply(&mut g.legend);
        for (i, slot) in [&self.axes.x, &self.axes.y, &self.axes.altx, &self.axes.alty]
            .into_iter()
            .enumerate()
        {
            if let Some(doc) = slot {
                doc.apply(&mut g.axes[i]);
            }
        }
        g.sets = self.sets.into_iter().map(|s| s.into_model(defaults)).collect();
        g
    }
}

impl SetDoc {
    fn from_model(s: &Set) -> SetDoc {
        SetDoc {
            kind: s.set_type,
            hidden: s.hidden,
            legend: s.legend.clone(),
            comment: s.comment.clone(),
            line: LineDoc::from_model(s),
            symbol: SymbolDoc::from_model(s),
            fill: FillDoc::from_model(s),
            error_bars: ErrBarDoc::from_model(&s.errbar),
            value_labels: AValueDoc::from_model(&s.avalue),
            data: data_rows(s),
        }
    }

    fn into_model(self, defaults: &Defaults) -> Set {
        let mut s = Set::with_defaults(defaults);
        s.set_type = self.kind;
        s.hidden = self.hidden;
        s.legend = self.legend;
        s.comment = self.comment;
        self.line.apply(&mut s);
        self.symbol.apply(&mut s);
        self.fill.apply(&mut s);
        self.error_bars.apply(&mut s.errbar);
        self.value_labels.apply(&mut s.avalue);
        let (cols, strs) = parse_rows(&self.data);
        let data = Arc::make_mut(&mut s.data);
        data.cols = cols;
        data.strs = strs;
        s
    }
}

/// Serialize a set's data as `.agr`-style rows (one point per line,
/// shortest round-trip floats, per-point strings quoted at the end).
fn data_rows(s: &Set) -> String {
    let npts = s.data.len();
    if npts == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(npts * 16);
    out.push('\n');
    for i in 0..npts {
        let mut row = String::new();
        for col in &s.data.cols {
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(&crate::write::n(col[i]));
        }
        if let Some(Some(label)) = s.data.strs.get(i) {
            row.push(' ');
            row.push_str(&crate::write::q(label));
        }
        out.push_str(&row);
        out.push('\n');
    }
    out
}

/// Parse a data block back into columns + optional per-point strings
/// (same row grammar as `.agr`; blank lines and `#` comments skipped).
#[allow(clippy::type_complexity)]
fn parse_rows(data: &str) -> (Vec<Vec<f64>>, Vec<Option<String>>) {
    let rows: Vec<(Vec<f64>, Option<String>)> = data
        .lines()
        .filter_map(crate::parse::data::parse_row)
        .collect();
    let ncols = rows.iter().map(|(r, _)| r.len()).max().unwrap_or(0);
    let cols = (0..ncols)
        .map(|c| rows.iter().filter_map(|(r, _)| r.get(c).copied()).collect())
        .collect();
    let strs = if rows.iter().any(|(_, s)| s.is_some()) {
        rows.into_iter().map(|(_, s)| s).collect()
    } else {
        Vec::new()
    };
    (cols, strs)
}

fn parse_hex(rgb: &str) -> Option<(u8, u8, u8)> {
    let hex = rgb.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}
