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
//!
//! Covered by this slice: page, color-map overrides, per-graph kind /
//! world / view / scales / titles, the four axes (label + tick basics +
//! sides), and sets (line / symbol / fill styling including the opacity
//! channels) with inline data. Not yet covered (falls back to defaults,
//! use `.agr` for full fidelity meanwhile): legend, frame, annotation
//! objects, error bars, avalues, fonts, defaults, advanced axis props.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::format::OxgrError;
use crate::model::{
    Defaults, FillType, Graph, GraphType, LineType, Placement, Project, ScaleType, Set,
    SymbolType, World,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<ColorDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphs: Vec<GraphDoc>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Page {
    pub width: u32,
    pub height: u32,
}

impl Default for Page {
    fn default() -> Self {
        let p = Project::default();
        Page { width: p.page_width, height: p.page_height }
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
    #[serde(default, skip_serializing_if = "is_default")]
    pub title: Text,
    #[serde(default, skip_serializing_if = "is_default")]
    pub subtitle: Text,
    #[serde(default, skip_serializing_if = "is_default")]
    pub axes: Axes,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sets: Vec<SetDoc>,
}

/// A rectangle span, used for both the world window and the viewport.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
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
pub struct AxisDoc {
    pub active: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Major tick spacing in world units.
    pub major: f64,
    /// Minor intervals per major.
    pub minor_ticks: i32,
    /// Which frame side carries ticks.
    pub ticks_on: Placement,
    /// Which frame side carries tick labels.
    pub labels_on: Placement,
}

impl Default for AxisDoc {
    fn default() -> Self {
        AxisDoc::from_model(&crate::model::Axis::default())
    }
}

impl AxisDoc {
    fn from_model(a: &crate::model::Axis) -> Self {
        AxisDoc {
            active: a.active,
            label: a.label.clone(),
            major: a.major,
            minor_ticks: a.minor_ticks,
            ticks_on: placement_of(a.op),
            labels_on: placement_of(a.tl_op),
        }
    }

    fn apply(&self, a: &mut crate::model::Axis) {
        a.active = self.active;
        a.label = self.label.clone();
        a.major = self.major;
        a.minor_ticks = self.minor_ticks;
        a.op = placement_code(self.ticks_on);
        a.tl_op = placement_code(self.labels_on);
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

// -------------------------------------------------------------------- sets

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct SetDoc {
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: crate::model::SetType,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub legend: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub line: LineDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub symbol: SymbolDoc,
    #[serde(default, skip_serializing_if = "is_default")]
    pub fill: FillDoc,
    /// Inline data: whitespace-separated columns, one point per row, an
    /// optional trailing quoted string per point — the `.agr` data block
    /// as a raw string.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data: String,
}

/// The connecting line (Grace line pen).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LineDoc {
    pub kind: LineType,
    pub dash: Dash,
    pub width: f64,
    pub color: i32,
    /// 0..=255 (QtGrace pen alpha).
    pub opacity: i32,
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
        }
    }

    fn apply(&self, s: &mut Set) {
        s.line_type = self.kind;
        s.linestyle = self.dash.code();
        s.linewidth = self.width;
        s.line_pen.color = self.color;
        s.line_pen.alpha = self.opacity;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    /// Draw every (skip+1)-th symbol.
    pub skip: i32,
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
            skip: s.symskip,
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
        s.symskip = self.skip;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

// ------------------------------------------------------------- conversions

impl Document {
    /// Build the v1 document mirror of a project (the covered slice).
    pub fn from_project(p: &Project) -> Document {
        Document {
            format: 1,
            page: Page { width: p.page_width, height: p.page_height },
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
        }
    }

    /// Materialize a project from the document.
    pub fn into_project(self) -> Result<Project, OxgrError> {
        let mut p = Project::default();
        p.page_width = self.page.width;
        p.page_height = self.page.height;
        for c in &self.colors {
            let rgb = parse_hex(&c.rgb)
                .ok_or_else(|| OxgrError::Color { id: c.id, rgb: c.rgb.clone() })?;
            p.color_overrides.push((c.id, rgb));
        }
        let defaults = p.defaults;
        p.graphs = self.graphs.into_iter().map(|g| g.into_model(&defaults)).collect();
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
        let mut g = Graph::default();
        g.graph_type = self.kind;
        g.hidden = self.hidden;
        g.world = World {
            xmin: self.world.xmin,
            xmax: self.world.xmax,
            ymin: self.world.ymin,
            ymax: self.world.ymax,
        };
        g.view.xmin = self.view.xmin;
        g.view.xmax = self.view.xmax;
        g.view.ymin = self.view.ymin;
        g.view.ymax = self.view.ymax;
        g.xscale = self.xscale;
        g.yscale = self.yscale;
        g.labels.title = self.title.text;
        g.labels.title_size = self.title.size;
        g.labels.title_font = self.title.font;
        g.labels.title_color = self.title.color;
        g.labels.subtitle = self.subtitle.text;
        g.labels.subtitle_size = self.subtitle.size;
        g.labels.subtitle_font = self.subtitle.font;
        g.labels.subtitle_color = self.subtitle.color;
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
            line: LineDoc::from_model(s),
            symbol: SymbolDoc::from_model(s),
            fill: FillDoc::from_model(s),
            data: data_rows(s),
        }
    }

    fn into_model(self, defaults: &Defaults) -> Set {
        let mut s = Set::with_defaults(defaults);
        s.set_type = self.kind;
        s.hidden = self.hidden;
        s.legend = self.legend;
        self.line.apply(&mut s);
        self.symbol.apply(&mut s);
        self.fill.apply(&mut s);
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
