//! Line-oriented `.agr`/`.xvg` reader.
//!
//! Classifies each line as a `@`-command, a data row, a `&` data-block
//! terminator, a comment (`#`) or blank, then drives the [`grammar`] parser and
//! applies the resulting [`Command`]s to a [`Project`] through a small mutable
//! parse cursor.

use crate::model::{Project, ScaleType, SetType};
use crate::parse::data;
use crate::parse::grammar::{
    self, AxisProp, Bound, Command, DefaultProp, FrameProp, LegendProp, SetProp, TextProp,
    TickLevelProp, ViewSpec, WorldSpec,
};

/// Mutable state carried while reading a file.
struct Cursor {
    /// Graph that bare graph-scoped commands apply to (set by `@with gN`).
    current_graph: usize,
    /// Dataset that data rows / `@type` flow into (set by `@target`).
    target: Option<(usize, usize)>,
    /// Next implicit set index when no `@target` is active (old-format files
    /// stream successive `&`-separated blocks into successive sets).
    auto_set: usize,
    /// Dataset type for the data currently being read.
    data_type: SetType,
    /// Accumulated numeric rows for the current data block.
    rows: Vec<Vec<f64>>,
    /// Whether each graph's world window was set explicitly (vs. needs autoscale).
    world_set: Vec<bool>,
    /// Project file format version (`@version`), 0 if unspecified.
    version: i32,
}

impl Cursor {
    fn new() -> Self {
        Cursor {
            current_graph: 0,
            target: None,
            auto_set: 0,
            data_type: SetType::Xy,
            rows: Vec::new(),
            world_set: Vec::new(),
            version: 0,
        }
    }

    fn mark_world_set(&mut self, graph: usize) {
        while self.world_set.len() <= graph {
            self.world_set.push(false);
        }
        self.world_set[graph] = true;
    }
}

/// Parse the textual contents of a `.agr`/`.xvg` file into a [`Project`].
pub fn parse_project(content: &str) -> Project {
    let mut project = Project::default();
    let mut cur = Cursor::new();

    for raw in content.lines() {
        let line = raw.trim_end_matches(['\r', '\n']);
        let trimmed = line.trim_start();

        if trimmed.is_empty() {
            flush_data(&mut project, &mut cur);
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(body) = trimmed.strip_prefix('@') {
            // A new command terminates any open data block.
            flush_data(&mut project, &mut cur);
            let cmd = grammar::parse_line(body.trim_start());
            apply(&mut project, &mut cur, cmd);
            continue;
        }
        if trimmed.starts_with('&') {
            flush_data(&mut project, &mut cur);
            continue;
        }
        // Otherwise a data row.
        if let Some(row) = data::parse_row(trimmed) {
            cur.rows.push(row);
        }
    }
    flush_data(&mut project, &mut cur);

    postprocess_version(&mut project, cur.version);
    autoscale_unset(&mut project, &cur);
    project
}

/// Apply version-dependent fixups for old file formats, mirroring Grace's
/// `postprocess_project` (`graphs.cpp`). Old ACE/gr files stored viewports as
/// normalized device coordinates (filling the page in both axes); Grace's
/// current coordinate system is isotropic (both axes scaled by the shorter
/// page side). For such files Grace forces US-Letter and rescales every
/// viewport by the page's per-axis extent so the plot still fills the page.
fn postprocess_version(project: &mut Project, version: i32) {
    if version == 0 {
        return;
    }
    // Pre-4.0.5 files are laid out on a US-Letter page.
    if version < 40005 {
        project.page_width = 792;
        project.page_height = 612;
    }
    // Up to 4.1.02 viewports are normalized-device-coordinates and must be
    // stretched into the isotropic system.
    if version <= 40102 {
        let w = project.page_width as f64;
        let h = project.page_height as f64;
        let (ex, ey) = if w < h { (1.0, h / w) } else { (w / h, 1.0) };
        for graph in &mut project.graphs {
            graph.view.xmin *= ex;
            graph.view.xmax *= ex;
            graph.view.ymin *= ey;
            graph.view.ymax *= ey;
            if graph.legend.loctype_view {
                graph.legend.x *= ex;
                graph.legend.y *= ey;
            }
        }
    }
}

/// Write the accumulated data rows into the target dataset, then clear them.
fn flush_data(project: &mut Project, cur: &mut Cursor) {
    if cur.rows.is_empty() {
        return;
    }
    let rows = std::mem::take(&mut cur.rows);
    // With an explicit `@target` use it; otherwise (old-format files) stream
    // each successive block into the next set of the current graph.
    let (g, s) = match cur.target {
        Some(t) => t,
        None => {
            let s = cur.auto_set;
            cur.auto_set += 1;
            (cur.current_graph, s)
        }
    };
    let ncols = cur
        .data_type
        .ncols()
        .min(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    let defaults = project.defaults;
    let graph = project.graph_mut(g);
    let set = graph.set_mut(s, &defaults);
    set.set_type = cur.data_type;
    set.data.cols = (0..ncols)
        .map(|c| rows.iter().filter_map(|r| r.get(c).copied()).collect())
        .collect();
}

/// Apply a parsed command to the project, updating the cursor as needed.
fn apply(project: &mut Project, cur: &mut Cursor, cmd: Command) {
    match cmd {
        Command::Unknown => {}
        Command::Version(v) => cur.version = v,
        Command::PageSize(w, h) => {
            if w >= 1.0 && h >= 1.0 {
                project.page_width = w.round() as u32;
                project.page_height = h.round() as u32;
            }
        }
        Command::With { graph, set } => {
            cur.current_graph = graph;
            // `@with gN.sM` directs data into that set; a bare `@with gN`
            // clears the explicit target and resets the implicit set counter,
            // so streamed blocks start at set 0 of the new graph.
            cur.target = set.map(|s| (graph, s));
            if set.is_none() {
                cur.auto_set = 0;
            }
        }
        Command::Target { graph, set } => {
            cur.current_graph = graph;
            cur.target = Some((graph, set));
            let defaults = project.defaults;
            project.graph_mut(graph).set_mut(set, &defaults);
        }
        Command::TypeDecl(t) => {
            cur.data_type = t;
            if let Some((g, s)) = cur.target {
                let defaults = project.defaults;
                project.graph_mut(g).set_mut(s, &defaults).set_type = t;
            }
        }
        Command::GraphOnOff { graph, on } => project.graph_mut(graph).hidden = !on,
        Command::GraphHidden { graph, hidden } => project.graph_mut(graph).hidden = hidden,
        Command::GraphType { graph, ty } => project.graph_mut(graph).graph_type = ty,
        Command::GraphBargap { graph, gap } => project.graph_mut(graph).bargap = gap,
        Command::GraphStacked { graph, on } => project.graph_mut(graph).stacked = on,
        Command::World(spec) => {
            let g = cur.current_graph;
            apply_world(project.graph_mut(g), spec);
            cur.mark_world_set(g);
        }
        Command::View(spec) => apply_view(project.graph_mut(cur.current_graph), spec),
        Command::Znorm(z) => project.graph_mut(cur.current_graph).znorm = z,
        Command::Default(p) => apply_default(&mut project.defaults, p),
        Command::Axis { axis, prop } => {
            let g = cur.current_graph;
            apply_axis(&mut project.graph_mut(g).axes[axis.index()], prop);
        }
        Command::AxesScale { x, scale } => set_scale(project.graph_mut(cur.current_graph), x, scale),
        Command::AxesInvert { x, on } => {
            let graph = project.graph_mut(cur.current_graph);
            if x {
                graph.xinvert = on;
            } else {
                graph.yinvert = on;
            }
        }
        Command::Set { set, prop } => {
            let defaults = project.defaults;
            let g = cur.current_graph;
            apply_set(project.graph_mut(g).set_mut(set, &defaults), prop);
        }
        Command::Frame(p) => apply_frame(&mut project.graph_mut(cur.current_graph).frame, p),
        Command::Title(p) => apply_title(&mut project.graph_mut(cur.current_graph).labels, p, true),
        Command::Subtitle(p) => {
            apply_title(&mut project.graph_mut(cur.current_graph).labels, p, false)
        }
        Command::Legend(p) => apply_legend(&mut project.graph_mut(cur.current_graph).legend, p),
        Command::MapColor { index, rgb } => {
            project.color_overrides.retain(|&(i, _)| i != index);
            project.color_overrides.push((index, rgb));
        }
    }
}

fn apply_world(graph: &mut crate::model::Graph, spec: WorldSpec) {
    match spec {
        WorldSpec::Full(a, b, c, d) => {
            graph.world.xmin = a;
            graph.world.ymin = b;
            graph.world.xmax = c;
            graph.world.ymax = d;
        }
        WorldSpec::Component(bound, v) => match bound {
            Bound::Xmin => graph.world.xmin = v,
            Bound::Xmax => graph.world.xmax = v,
            Bound::Ymin => graph.world.ymin = v,
            Bound::Ymax => graph.world.ymax = v,
        },
    }
}

fn apply_view(graph: &mut crate::model::Graph, spec: ViewSpec) {
    match spec {
        ViewSpec::Full(a, b, c, d) => {
            graph.view.xmin = a;
            graph.view.ymin = b;
            graph.view.xmax = c;
            graph.view.ymax = d;
        }
        ViewSpec::Component(bound, v) => match bound {
            Bound::Xmin => graph.view.xmin = v,
            Bound::Xmax => graph.view.xmax = v,
            Bound::Ymin => graph.view.ymin = v,
            Bound::Ymax => graph.view.ymax = v,
        },
    }
}

fn apply_default(d: &mut crate::model::Defaults, p: DefaultProp) {
    match p {
        DefaultProp::Linestyle(n) => d.linestyle = n,
        DefaultProp::Linewidth(n) => d.linewidth = n,
        DefaultProp::Color(n) => d.color = n,
        DefaultProp::Pattern(n) => d.pattern = n,
        DefaultProp::CharSize(n) => d.charsize = n,
        DefaultProp::Font(n) if n >= 0 => d.font = n,
        DefaultProp::Font(_) => {} // ignored sentinel (e.g. "font source")
        DefaultProp::SymbolSize(n) => d.symsize = n,
    }
}

fn set_scale(graph: &mut crate::model::Graph, x: bool, scale: ScaleType) {
    if x {
        graph.xscale = scale;
    } else {
        graph.yscale = scale;
    }
}

fn apply_axis(axis: &mut crate::model::Axis, prop: AxisProp) {
    match prop {
        AxisProp::Active(b) => axis.active = b,
        AxisProp::BarActive(b) => axis.draw_bar = b,
        AxisProp::BarColor(n) => axis.bar_color = n,
        AxisProp::BarLinestyle(n) => axis.bar_linestyle = n,
        AxisProp::BarLinewidth(n) => axis.bar_linewidth = n,
        AxisProp::LabelText(s) => axis.label = s,
        AxisProp::LabelFont(n) => axis.label_font = n,
        AxisProp::LabelColor(n) => axis.label_color = n,
        AxisProp::LabelCharSize(n) => axis.label_charsize = n,
        AxisProp::TicksActive(b) => axis.ticks = b,
        AxisProp::TicksDir(inn) => axis.ticks_in = inn,
        AxisProp::MinorTicks(n) => axis.minor_ticks = n,
        AxisProp::AutoNum(n) => axis.autonum = n,
        AxisProp::Major(p) => apply_tick_level(&mut axis.major_props, &mut axis.major, p),
        AxisProp::Minor(p) => apply_tick_level(&mut axis.minor_props, &mut axis.major, p),
        AxisProp::TlActive(b) => axis.ticklabels = b,
        AxisProp::TlPrec(n) => axis.tl_prec = n,
        AxisProp::TlFormat(f) => axis.tl_format = f,
        AxisProp::TlFont(n) => axis.tl_font = n,
        AxisProp::TlColor(n) => axis.tl_color = n,
        AxisProp::TlCharSize(n) => axis.tl_charsize = n,
        AxisProp::TlAngle(n) => axis.tl_angle = n,
        AxisProp::TlAppend(s) => axis.tl_append = s,
        AxisProp::TlPrepend(s) => axis.tl_prepend = s,
        AxisProp::Ignored => {}
    }
}

fn apply_tick_level(props: &mut crate::model::TickProps, major_spacing: &mut f64, p: TickLevelProp) {
    match p {
        TickLevelProp::Spacing(v) => *major_spacing = v,
        TickLevelProp::Size(v) => props.size = v,
        TickLevelProp::Color(n) => props.color = n,
        TickLevelProp::Linewidth(n) => props.linewidth = n,
        TickLevelProp::Linestyle(n) => props.linestyle = n,
        TickLevelProp::Grid(b) => props.grid = b,
    }
}

fn apply_set(set: &mut crate::model::Set, prop: SetProp) {
    use crate::model::{FillType, LineType, SymbolType};
    match prop {
        SetProp::Type(t) => set.set_type = t,
        SetProp::Hidden(b) => set.hidden = b,
        SetProp::Symbol(n) => set.symbol = SymbolType::from_code(n),
        SetProp::SymbolSize(n) => set.symbol_size = n,
        SetProp::SymbolColor(n) => set.symbol_pen.color = n,
        SetProp::SymbolFillColor(n) => set.symbol_fill.color = n,
        SetProp::SymbolFillPattern(n) => set.symbol_fill.pattern = n,
        SetProp::SymbolLinewidth(n) => set.symbol_linewidth = n,
        SetProp::SymbolLinestyle(n) => set.symbol_linestyle = n,
        SetProp::LineType(n) => set.line_type = LineType::from_code(n),
        SetProp::LineColor(n) => set.line_pen.color = n,
        SetProp::LineLinewidth(n) => set.linewidth = n,
        SetProp::LineLinestyle(n) => set.linestyle = n,
        SetProp::Color(n) => {
            set.line_pen.color = n;
            set.symbol_pen.color = n;
        }
        SetProp::Linewidth(n) => set.linewidth = n,
        SetProp::Linestyle(n) => set.linestyle = n,
        SetProp::FillType(n) => {
            set.fill_type = match n {
                1 => FillType::Polygon,
                2 => FillType::Baseline,
                _ => FillType::None,
            }
        }
        SetProp::FillColor(n) => set.fill_pen.color = n,
        SetProp::FillPattern(n) => set.fill_pen.pattern = n,
        SetProp::BaselineType(n) => set.baseline_type = n,
        SetProp::Dropline(b) => set.dropline = b,
        SetProp::Legend(s) => set.legend = s,
        SetProp::Comment(s) => set.comment = s,
        SetProp::Ignored => {}
    }
}

fn apply_frame(frame: &mut crate::model::Frame, p: FrameProp) {
    match p {
        FrameProp::Type(n) => frame.frame_type = n,
        FrameProp::Linestyle(n) => frame.linestyle = n,
        FrameProp::Linewidth(n) => frame.linewidth = n,
        FrameProp::Color(n) => frame.pen.color = n,
        FrameProp::Pattern(n) => frame.pen.pattern = n,
        FrameProp::BackgroundColor(n) => frame.fill_pen.color = n,
        FrameProp::BackgroundPattern(n) => {
            frame.fill_pen.pattern = n;
            frame.fill = n != 0;
        }
    }
}

fn apply_title(labels: &mut crate::model::Labels, p: TextProp, is_title: bool) {
    match (is_title, p) {
        (true, TextProp::Text(s)) => labels.title = s,
        (true, TextProp::Font(n)) => labels.title_font = n,
        (true, TextProp::Size(n)) => labels.title_size = n,
        (true, TextProp::Color(n)) if n >= 0 => labels.title_color = n,
        (false, TextProp::Text(s)) => labels.subtitle = s,
        (false, TextProp::Font(n)) => labels.subtitle_font = n,
        (false, TextProp::Size(n)) => labels.subtitle_size = n,
        (false, TextProp::Color(n)) if n >= 0 => labels.subtitle_color = n,
        _ => {}
    }
}

fn apply_legend(legend: &mut crate::model::Legend, p: LegendProp) {
    match p {
        LegendProp::Active(b) => legend.active = b,
        LegendProp::LoctypeView(v) => legend.loctype_view = v,
        LegendProp::Position(x, y) => {
            legend.x = x;
            legend.y = y;
        }
        LegendProp::X(x) => legend.x = x,
        LegendProp::Y(y) => legend.y = y,
        LegendProp::Font(n) => legend.font = n,
        LegendProp::Color(n) => legend.color = n,
        LegendProp::CharSize(n) => legend.charsize = n,
        LegendProp::Length(n) => legend.length = n,
        LegendProp::Vgap(n) => legend.vgap = n,
        LegendProp::Hgap(n) => legend.hgap = n,
        LegendProp::Invert(b) => legend.invert = b,
        LegendProp::BoxOn(b) => legend.box_on = b,
        LegendProp::BoxColor(n) => legend.box_color = n,
        LegendProp::BoxLinewidth(n) => legend.box_linewidth = n,
        LegendProp::BoxLinestyle(n) => legend.box_linestyle = n,
        LegendProp::BoxFillColor(n) => legend.box_fill_color = n,
        LegendProp::BoxFillPattern(n) => legend.box_fill_pattern = n,
        LegendProp::Ignored => {}
    }
}

/// Autoscale graphs whose world window was never set, using their data extents.
fn autoscale_unset(project: &mut Project, cur: &Cursor) {
    for (gi, graph) in project.graphs.iter_mut().enumerate() {
        if cur.world_set.get(gi).copied().unwrap_or(false) {
            continue;
        }
        let mut xmin = f64::INFINITY;
        let mut xmax = f64::NEG_INFINITY;
        let mut ymin = f64::INFINITY;
        let mut ymax = f64::NEG_INFINITY;
        let mut any = false;
        for set in &graph.sets {
            if let (Some(xs), Some(ys)) = (set.data.x(), set.data.y()) {
                for &x in xs {
                    if x.is_finite() {
                        xmin = xmin.min(x);
                        xmax = xmax.max(x);
                        any = true;
                    }
                }
                for &y in ys {
                    if y.is_finite() {
                        ymin = ymin.min(y);
                        ymax = ymax.max(y);
                    }
                }
            }
        }
        if !any {
            continue;
        }
        // Pad degenerate ranges so the transform stays finite.
        if (xmax - xmin).abs() < f64::EPSILON {
            xmin -= 0.5;
            xmax += 0.5;
        }
        if (ymax - ymin).abs() < f64::EPSILON {
            ymin -= 0.5;
            ymax += 0.5;
        }
        graph.world.xmin = xmin;
        graph.world.xmax = xmax;
        graph.world.ymin = ymin;
        graph.world.ymax = ymax;
    }
}
