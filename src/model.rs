//! The in-memory plot model: a [`Project`] holds a page and a list of
//! [`Graph`]s; each graph holds [`Set`]s (datasets), four [`Axis`]es, a
//! [`Frame`], a [`Legend`] and titles.
//!
//! The structures mirror Grace/QtGrace state (see `src/graphs.h`,
//! `src/defines.h`) but keep only what the renderer needs, with idiomatic
//! Rust types. Fields default to Grace's documented defaults via [`Default`].

pub mod defaults;
pub mod enums;

pub use defaults::Defaults;
pub use enums::{
    AxisId, FillType, GraphType, LineType, Placement, ScaleType, SetType, SymbolType, TickFormat,
};

/// A pen: color index plus fill pattern and alpha, as in Grace's `Pen`.
#[derive(Debug, Clone, Copy)]
pub struct Pen {
    pub color: i32,
    pub pattern: i32,
    pub alpha: i32,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            color: 1,
            pattern: 1,
            alpha: 255,
        }
    }
}

/// World-coordinate window: the data range mapped onto the viewport.
#[derive(Debug, Clone, Copy)]
pub struct World {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

impl Default for World {
    fn default() -> Self {
        World {
            xmin: 0.0,
            xmax: 1.0,
            ymin: 0.0,
            ymax: 1.0,
        }
    }
}

/// Viewport: the graph's rectangle on the page in normalized (0..1) coords.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

impl Default for View {
    fn default() -> Self {
        View {
            xmin: 0.15,
            xmax: 0.85,
            ymin: 0.15,
            ymax: 0.85,
        }
    }
}

/// Properties of one tick level (major or minor).
#[derive(Debug, Clone, Copy)]
pub struct TickProps {
    /// Tick mark length (Grace units).
    pub size: f64,
    pub color: i32,
    pub linewidth: f64,
    pub linestyle: i32,
    /// Draw grid lines across the frame at these ticks.
    pub grid: bool,
}

impl Default for TickProps {
    fn default() -> Self {
        TickProps {
            size: 1.0,
            color: 1,
            linewidth: 1.0,
            linestyle: 1,
            grid: false,
        }
    }
}

/// One of a graph's axes plus its tick marks and labels (Grace `tickmarks`).
#[derive(Debug, Clone)]
pub struct Axis {
    /// Whether the axis (and its ticks) is drawn.
    pub active: bool,
    /// Axis label text (plain; markup handled at draw time).
    pub label: String,
    pub label_font: i32,
    pub label_charsize: f64,
    pub label_color: i32,
    /// Draw the axis bar (the line along the frame edge).
    pub draw_bar: bool,
    pub bar_color: i32,
    pub bar_linestyle: i32,
    pub bar_linewidth: f64,
    /// Whether tick marks are drawn at all.
    pub ticks: bool,
    /// Spacing between major ticks in world units.
    pub major: f64,
    /// Number of minor intervals between consecutive major ticks.
    pub minor_ticks: i32,
    /// Approximate desired number of major ticks (for autotick; unused in M1).
    pub autonum: i32,
    /// Round the first major tick down to a multiple of the spacing
    /// (`tick place rounded`, Grace `t_round`; default true).
    pub tick_round: bool,
    /// Tick direction (`tick in|out|both`): 0 in, 1 out, 2 both.
    pub tick_inout: i32,
    /// Bar/tick placement (`tick op`): 0 normal edge, 1 opposite, 2 both.
    pub op: i32,
    /// Tick label placement (`ticklabel op`).
    pub tl_op: i32,
    /// Axis label placement (`label op`).
    pub label_op: i32,
    /// Axis sits at world zero of the perpendicular coordinate
    /// (`type zero`; drawticks.cpp `t->zero`). Skipped when 0 is outside
    /// the world window.
    pub zero: bool,
    /// Outward shifts of the normal / opposite axis positions in view units
    /// (`axis offset X , Y`; drawticks.cpp `offsx`/`offsy`).
    pub offs_normal: f64,
    pub offs_opposite: f64,
    pub major_props: TickProps,
    pub minor_props: TickProps,
    /// Whether numeric tick labels are drawn.
    pub ticklabels: bool,
    pub tl_format: TickFormat,
    pub tl_prec: i32,
    pub tl_font: i32,
    pub tl_charsize: f64,
    pub tl_color: i32,
    pub tl_angle: i32,
    pub tl_prepend: String,
    pub tl_append: String,
    /// Restrict tick labels to `tl_start..=tl_stop` when the respective type is
    /// "spec" (Grace `ticklabel start/stop type spec`); ticks themselves are
    /// still drawn at every position.
    /// Label every (skip+1)-th major tick (`ticklabel skip`).
    pub tl_skip: i32,
    /// Transform applied to major tick values before formatting
    /// (`ticklabel formula "$t-273.15"`); empty = identity.
    pub tl_formula: String,
    /// Specified ticks (Grace `TICKS_SPEC_*`): 0 = generated, 1 = positions
    /// from `spec_ticks`, 2 = positions and labels from `spec_ticks`.
    pub spec_type: i32,
    /// Number of specified ticks in use (`tick spec N`).
    pub spec_count: usize,
    /// Index-addressed specified ticks (`tick major IDX, POS`,
    /// `ticklabel IDX, "label"`).
    pub spec_ticks: Vec<SpecTick>,
    pub tl_start_spec: bool,
    pub tl_start: f64,
    pub tl_stop_spec: bool,
    pub tl_stop: f64,
}

impl Default for Axis {
    fn default() -> Self {
        Axis {
            active: true,
            label: String::new(),
            label_font: 4,
            label_charsize: 1.0,
            label_color: 1,
            draw_bar: true,
            bar_color: 1,
            bar_linestyle: 1,
            bar_linewidth: 1.0,
            ticks: true,
            major: 0.5,
            minor_ticks: 1,
            autonum: 6,
            tick_round: true,
            tick_inout: 0,
            op: 2,
            tl_op: 0,
            label_op: 0,
            zero: false,
            offs_normal: 0.0,
            offs_opposite: 0.0,
            major_props: TickProps::default(),
            minor_props: TickProps {
                size: 0.5,
                ..TickProps::default()
            },
            ticklabels: true,
            tl_format: TickFormat::Decimal,
            tl_prec: 1,
            tl_font: 4,
            tl_charsize: 1.0,
            tl_color: 1,
            tl_angle: 0,
            tl_prepend: String::new(),
            tl_append: String::new(),
            tl_skip: 0,
            tl_formula: String::new(),
            spec_type: 0,
            spec_count: 0,
            spec_ticks: Vec::new(),
            tl_start_spec: false,
            tl_start: 0.0,
            tl_stop_spec: false,
            tl_stop: 0.0,
        }
    }
}

/// One explicitly specified tick (Grace `tloc[]`).
#[derive(Debug, Clone, Default)]
pub struct SpecTick {
    pub pos: f64,
    /// Major (true) or minor tick mark.
    pub major: bool,
    /// Custom label (used when the axis `spec_type` is 2).
    pub label: Option<String>,
}

impl Axis {
    /// Mutable spec tick at `idx`, growing the list as needed.
    pub fn spec_tick_mut(&mut self, idx: usize) -> &mut SpecTick {
        while self.spec_ticks.len() <= idx {
            self.spec_ticks.push(SpecTick::default());
        }
        &mut self.spec_ticks[idx]
    }
}

/// The box drawn around the plotting area.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// 0 = closed box, others (half-open variants) unused in M1.
    pub frame_type: i32,
    pub pen: Pen,
    pub linestyle: i32,
    pub linewidth: f64,
    /// Background fill behind the frame.
    pub fill_pen: Pen,
    /// Whether the background fill is applied.
    pub fill: bool,
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            frame_type: 0,
            pen: Pen::default(),
            linestyle: 1,
            linewidth: 1.0,
            fill_pen: Pen {
                color: 0,
                pattern: 0,
                alpha: 255,
            },
            fill: false,
        }
    }
}

/// Title and subtitle text + styling.
#[derive(Debug, Clone)]
pub struct Labels {
    pub title: String,
    pub title_font: i32,
    pub title_size: f64,
    pub title_color: i32,
    pub subtitle: String,
    pub subtitle_font: i32,
    pub subtitle_size: f64,
    pub subtitle_color: i32,
}

impl Default for Labels {
    fn default() -> Self {
        Labels {
            title: String::new(),
            title_font: 4,
            title_size: 1.5,
            title_color: 1,
            subtitle: String::new(),
            subtitle_font: 4,
            subtitle_size: 1.0,
            subtitle_color: 1,
        }
    }
}

/// Legend placement and styling.
#[derive(Debug, Clone)]
pub struct Legend {
    pub active: bool,
    /// Position in view coordinates if `loctype_view`, else world coordinates.
    pub loctype_view: bool,
    /// Top-left anchor of the legend.
    pub x: f64,
    pub y: f64,
    pub font: i32,
    pub charsize: f64,
    pub color: i32,
    /// Swatch line length (Grace `legend length`, in 0.01 view units).
    pub length: f64,
    /// Vertical gap between entries (0.01 view units).
    pub vgap: f64,
    /// Horizontal gap (0.01 view units).
    pub hgap: f64,
    /// Reverse entry order.
    pub invert: bool,
    /// Draw the surrounding box.
    pub box_on: bool,
    pub box_color: i32,
    pub box_linewidth: f64,
    pub box_linestyle: i32,
    pub box_fill_color: i32,
    pub box_fill_pattern: i32,
}

impl Default for Legend {
    fn default() -> Self {
        Legend {
            active: false,
            loctype_view: true,
            x: 0.8,
            y: 0.8,
            font: 0,
            charsize: 1.0,
            color: 1,
            length: 4.0,
            vgap: 1.0,
            hgap: 1.0,
            invert: false,
            box_on: true,
            box_color: 1,
            box_linewidth: 1.0,
            box_linestyle: 1,
            box_fill_color: 0,
            box_fill_pattern: 1,
        }
    }
}

/// Numeric (and optional string) data columns for a dataset.
///
/// Column meaning depends on [`Set::set_type`]: `cols[0]` is always X,
/// `cols[1]` Y, and further columns hold errors/sizes/etc.
#[derive(Debug, Clone, Default)]
pub struct Dataset {
    pub cols: Vec<Vec<f64>>,
    /// Optional per-point strings (trailing quoted column in data rows),
    /// used by avalue type 4 (Grace `AVALUE_TYPE_STRING`).
    pub strs: Vec<Option<String>>,
}

impl Dataset {
    /// Number of data points (length of the shortest present column).
    pub fn len(&self) -> usize {
        self.cols.iter().map(|c| c.len()).min().unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// X / Y accessors (return `None` if the column is absent).
    pub fn x(&self) -> Option<&[f64]> {
        self.cols.first().map(|c| c.as_slice())
    }
    pub fn y(&self) -> Option<&[f64]> {
        self.cols.get(1).map(|c| c.as_slice())
    }
}

/// A dataset (`s<n>`) plus its visual styling.
#[derive(Debug, Clone)]
pub struct Set {
    pub hidden: bool,
    pub set_type: SetType,
    pub data: Dataset,
    pub legend: String,
    pub comment: String,

    pub symbol: SymbolType,
    pub symbol_size: f64,
    pub symbol_pen: Pen,
    pub symbol_fill: Pen,
    pub symbol_linewidth: f64,
    pub symbol_linestyle: i32,
    /// Character code for SYM_CHAR symbols (`symbol char`).
    pub symbol_char: u8,
    /// Font slot for SYM_CHAR symbols (`symbol char font`).
    pub symbol_char_font: i32,

    pub line_type: LineType,
    pub line_pen: Pen,
    pub linestyle: i32,
    pub linewidth: f64,

    pub fill_type: FillType,
    pub fill_pen: Pen,
    /// Polygon fill rule: 0 = winding, 1 = even-odd (Grace `FILLRULE_*`).
    pub fill_rule: i32,
    /// Baseline reference for baseline fills (0 = y=0, see Grace `setybase`).
    pub baseline_type: i32,
    /// Draw a vertical line from each point down to the baseline.
    pub dropline: bool,
    /// Annotated point values (Grace `AValue`).
    pub avalue: AValue,
    /// Error bars (Grace `Errbar`).
    pub errbar: ErrBar,
}

/// Error bar properties (`s errorbar ...`; defaults from `set_default_errbar`,
/// defaults.cpp — active by default, placement "both").
#[derive(Debug, Clone)]
pub struct ErrBar {
    pub active: bool,
    /// 0 normal (plus side), 1 opposite, 2 both.
    pub place: i32,
    pub color: i32,
    /// Cap half-length factor (view length = 0.01 * size).
    pub size: f64,
    /// Cap line width/style.
    pub linewidth: f64,
    pub linestyle: i32,
    /// Riser line width/style.
    pub riser_linewidth: f64,
    pub riser_linestyle: i32,
    /// Clip overlong risers and finish them with an arrowhead.
    pub arrow_clip: bool,
    /// Maximum riser length in view units when clipping.
    pub cliplen: f64,
}

impl Default for ErrBar {
    fn default() -> Self {
        ErrBar {
            active: true,
            place: 2,
            color: 1,
            size: 1.0,
            linewidth: 1.0,
            linestyle: 1,
            riser_linewidth: 1.0,
            riser_linestyle: 1,
            arrow_clip: false,
            cliplen: 0.1,
        }
    }
}

/// Annotated-value labels drawn at each data point (`s avalue ...`,
/// Grace `AValue` / plotone.cpp `drawsetavalues`).
#[derive(Debug, Clone)]
pub struct AValue {
    pub active: bool,
    /// 0 none, 1 X, 2 Y, 3 XY, 4 per-point string, 5 Z column.
    pub avtype: i32,
    pub size: f64,
    pub font: i32,
    pub color: i32,
    /// Rotation angle in degrees (`avalue rot`).
    pub angle: f64,
    pub format: TickFormat,
    pub prec: i32,
    /// Offset from the data point in view units (`avalue offset`).
    pub offx: f64,
    pub offy: f64,
    pub prepend: String,
    pub append: String,
}

impl Default for AValue {
    fn default() -> Self {
        AValue {
            active: false,
            avtype: 2,
            size: 1.0,
            font: 0,
            color: 1,
            angle: 0.0,
            format: TickFormat::General,
            prec: 3,
            offx: 0.0,
            offy: 0.0,
            prepend: String::new(),
            append: String::new(),
        }
    }
}

impl Set {
    /// Build a set seeded from the project defaults.
    pub fn with_defaults(d: &Defaults) -> Self {
        let pen = Pen {
            color: d.color,
            pattern: d.pattern,
            alpha: 255,
        };
        Set {
            hidden: false,
            set_type: SetType::Xy,
            data: Dataset::default(),
            legend: String::new(),
            comment: String::new(),
            symbol: SymbolType::None,
            symbol_size: d.symsize,
            symbol_pen: pen,
            symbol_fill: pen,
            symbol_linewidth: d.linewidth,
            symbol_linestyle: d.linestyle,
            symbol_char: 65,
            symbol_char_font: d.font,
            line_type: LineType::Straight,
            line_pen: pen,
            linestyle: d.linestyle,
            linewidth: d.linewidth,
            fill_type: FillType::None,
            fill_pen: pen,
            fill_rule: 0,
            baseline_type: 0,
            dropline: false,
            avalue: AValue::default(),
            errbar: ErrBar::default(),
        }
    }
}

/// A single graph: viewport, world window, sets, axes and decorations.
#[derive(Debug, Clone)]
pub struct Graph {
    pub hidden: bool,
    pub graph_type: GraphType,
    pub xscale: ScaleType,
    pub yscale: ScaleType,
    pub xinvert: bool,
    pub yinvert: bool,
    /// Stacked bar/chart flag.
    pub stacked: bool,
    /// Horizontal gap between bar groups in a chart (view units).
    pub bargap: f64,
    /// Normalization divisor for xysize symbol scaling (`@znorm`).
    pub znorm: f64,
    pub world: World,
    pub view: View,
    /// Four axes: X, Y, AltX, AltY (indexed via [`AxisId::index`]).
    pub axes: [Axis; 4],
    pub frame: Frame,
    pub labels: Labels,
    pub legend: Legend,
    pub sets: Vec<Set>,
}

impl Default for Graph {
    fn default() -> Self {
        let alt = |active: bool| Axis {
            active,
            ..Axis::default()
        };
        Graph {
            hidden: false,
            graph_type: GraphType::Xy,
            xscale: ScaleType::Normal,
            yscale: ScaleType::Normal,
            xinvert: false,
            yinvert: false,
            stacked: false,
            bargap: 0.0,
            znorm: 1.0,
            world: World::default(),
            view: View::default(),
            axes: [
                Axis::default(),
                Axis::default(),
                alt(false),
                alt(false),
            ],
            frame: Frame::default(),
            labels: Labels::default(),
            legend: Legend::default(),
            sets: Vec::new(),
        }
    }
}

impl Graph {
    /// Get a mutable set by index, growing the vector with defaulted sets.
    pub fn set_mut(&mut self, index: usize, defaults: &Defaults) -> &mut Set {
        while self.sets.len() <= index {
            self.sets.push(Set::with_defaults(defaults));
        }
        &mut self.sets[index]
    }
}

/// The whole plot: page geometry, default properties and all graphs.
#[derive(Debug, Clone)]
pub struct Project {
    /// Page width in pixels at [`Project::dpi`].
    pub page_width: u32,
    /// Page height in pixels.
    pub page_height: u32,
    pub dpi: f64,
    pub defaults: Defaults,
    /// Color map overrides (`@map color`): index -> (r, g, b).
    pub color_overrides: Vec<(i32, (u8, u8, u8))>,
    /// Font slot -> embedded face map (see [`crate::font::FontMap`]); set
    /// from `@version` and overridden per slot by `@map font`.
    pub font_map: crate::font::FontMap,
    pub graphs: Vec<Graph>,
    /// Annotation string objects (`@with string`).
    pub strings: Vec<StringObj>,
    /// Annotation line objects (`@with line`).
    pub lines: Vec<LineObj>,
    /// Annotation box objects (`@with box`).
    pub boxes: Vec<BoxObj>,
    /// Annotation ellipse objects (`@with ellipse`).
    pub ellipses: Vec<EllipseObj>,
    /// Page timestamp (`@timestamp …`). Grace refreshes the text to the
    /// current time when rendering; we draw the text stored in the file.
    pub timestamp: StringObj,
}

/// Annotation string (Grace `plotstr`, defaults from `set_default_string`).
#[derive(Debug, Clone)]
pub struct StringObj {
    pub active: bool,
    /// `true` = view coordinates, `false` = world coords of graph `gno`.
    pub loctype_view: bool,
    /// Graph the object is attached to (used when `loctype` is world).
    pub gno: usize,
    pub x: f64,
    pub y: f64,
    pub color: i32,
    /// Rotation angle in degrees, counter-clockwise.
    pub rot: f64,
    pub font: i32,
    /// Grace justification bits: h = just & 3 (0 left, 1 right, 2 center),
    /// v = just & 12 (0 baseline, 4 bottom, 8 top, 12 middle). `draw.h`.
    pub just: i32,
    pub charsize: f64,
    pub text: String,
}

impl StringObj {
    pub fn with_defaults(d: &Defaults) -> Self {
        StringObj {
            active: false,
            loctype_view: true,
            gno: 0,
            x: 0.0,
            y: 0.0,
            color: d.color,
            rot: 0.0,
            font: d.font,
            just: 0,
            charsize: d.charsize,
            text: String::new(),
        }
    }
}

/// Annotation line with optional arrowheads (Grace `linetype`).
#[derive(Debug, Clone)]
pub struct LineObj {
    pub active: bool,
    pub loctype_view: bool,
    pub gno: usize,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub linewidth: f64,
    pub linestyle: i32,
    pub color: i32,
    /// Which ends carry an arrowhead: 0 none, 1 start, 2 end, 3 both.
    pub arrow_end: i32,
    /// Arrowhead type: 0 open lines, 1 filled, 2 background-filled.
    pub arrow_type: i32,
    /// Arrowhead length factor (view length = `0.01 * length`).
    pub arrow_length: f64,
    /// Arrow layout form factors d/L and l/L (`set_default_arrow`: 1.0, 1.0).
    pub arrow_dl: f64,
    pub arrow_ll: f64,
}

impl LineObj {
    pub fn with_defaults(d: &Defaults) -> Self {
        LineObj {
            active: false,
            loctype_view: true,
            gno: 0,
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
            linewidth: d.linewidth,
            linestyle: d.linestyle,
            color: d.color,
            arrow_end: 0,
            arrow_type: 0,
            arrow_length: 1.0,
            arrow_dl: 1.0,
            arrow_ll: 1.0,
        }
    }
}

/// Annotation rectangle (Grace `boxtype`).
#[derive(Debug, Clone)]
pub struct BoxObj {
    pub active: bool,
    pub loctype_view: bool,
    pub gno: usize,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub linewidth: f64,
    pub linestyle: i32,
    pub color: i32,
    pub fill_color: i32,
    pub fill_pattern: i32,
}

impl BoxObj {
    pub fn with_defaults(d: &Defaults) -> Self {
        BoxObj {
            active: false,
            loctype_view: true,
            gno: 0,
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
            linewidth: d.linewidth,
            linestyle: d.linestyle,
            color: d.color,
            fill_color: d.color,
            fill_pattern: d.pattern,
        }
    }
}

/// Annotation ellipse, inscribed in its bounding rectangle (Grace
/// `ellipsetype` — same fields as a box).
pub type EllipseObj = BoxObj;

impl Default for Project {
    fn default() -> Self {
        Project {
            // QtGrace's hardcopy default for files without `@page size`:
            // US Letter landscape at 72 DPI (matches its PNG export).
            page_width: 792,
            page_height: 612,
            dpi: 72.0,
            defaults: Defaults::default(),
            color_overrides: Vec::new(),
            font_map: crate::font::FONT_MAP_DEFAULT,
            graphs: Vec::new(),
            strings: Vec::new(),
            lines: Vec::new(),
            boxes: Vec::new(),
            ellipses: Vec::new(),
            timestamp: StringObj {
                // Grace's default timestamp anchor (defaults.cpp).
                x: 0.03,
                y: 0.03,
                ..StringObj::with_defaults(&Defaults::default())
            },
        }
    }
}

impl Project {
    /// Get a mutable graph by index, creating defaulted graphs as needed.
    pub fn graph_mut(&mut self, index: usize) -> &mut Graph {
        while self.graphs.len() <= index {
            self.graphs.push(Graph::default());
        }
        &mut self.graphs[index]
    }
}
