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
    /// Ticks point inward (`true`) or outward.
    pub ticks_in: bool,
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
            ticks_in: true,
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
        }
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

/// Legend placement and styling (rendering deferred past M1).
#[derive(Debug, Clone)]
pub struct Legend {
    pub active: bool,
    /// Position in view coordinates if `loctype_view`, else world coordinates.
    pub loctype_view: bool,
    pub x: f64,
    pub y: f64,
    pub font: i32,
    pub charsize: f64,
    pub color: i32,
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

    pub line_type: LineType,
    pub line_pen: Pen,
    pub linestyle: i32,
    pub linewidth: f64,

    pub fill_type: FillType,
    pub fill_pen: Pen,
    /// Baseline reference for baseline fills (0 = y=0, see Grace `setybase`).
    pub baseline_type: i32,
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
            line_type: LineType::Straight,
            line_pen: pen,
            linestyle: d.linestyle,
            linewidth: d.linewidth,
            fill_type: FillType::None,
            fill_pen: pen,
            baseline_type: 0,
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
    pub graphs: Vec<Graph>,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            // Grace's DEFAULT_PAGE_WIDTH / DEFAULT_PAGE_HEIGHT at 72 DPI.
            page_width: 733,
            page_height: 538,
            dpi: 72.0,
            defaults: Defaults::default(),
            color_overrides: Vec::new(),
            graphs: Vec::new(),
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
