//! The property inspector: shows exactly the selected element's properties
//! (pages are element-exact — selecting a title shows only title rows),
//! headed by a clickable breadcrumb tracing the element's ancestry. All
//! pages are built from the shared row vocabulary in [`rows`], so they look
//! and behave identically.

pub mod axis;
pub mod graph;
pub mod legend;
pub mod object;
pub mod page;
pub mod rows;
pub mod set;

use oxygrace::{ElementId, Project};

use crate::edit::Edit;

/// Placement side options shared by several pages.
pub const SIDE_OPTS: [(i32, &str); 3] = [(0, "Normal"), (1, "Opposite"), (2, "Both")];

/// View/world coordinate anchoring options (objects, legend).
pub const LOCTYPE_OPTS: [(bool, &str); 2] = [(true, "View"), (false, "World")];

pub fn show(
    ui: &mut egui::Ui,
    project: &Project,
    selection: &mut Option<ElementId>,
    edits: &mut Vec<Edit>,
) {
    breadcrumb(ui, project, selection);
    ui.separator();
    // Dispatch on the (possibly breadcrumb-updated) selection, so a
    // breadcrumb click switches the page in the same frame.
    egui::ScrollArea::vertical().show(ui, |ui| match *selection {
        None => page::show(ui, project, edits),
        Some(ElementId::Graph(g)) => graph::area(ui, project, g, edits),
        Some(ElementId::Frame(g)) => graph::frame(ui, project, g, edits),
        Some(ElementId::Title(g)) => graph::title(ui, project, g, edits),
        Some(ElementId::Subtitle(g)) => graph::subtitle(ui, project, g, edits),
        Some(ElementId::AxisBar { graph, axis }) => axis::bar(ui, project, graph, axis, edits),
        Some(ElementId::TickLabels { graph, axis }) => {
            axis::tick_labels(ui, project, graph, axis, edits)
        }
        Some(ElementId::AxisLabel { graph, axis }) => axis::label(ui, project, graph, axis, edits),
        Some(ElementId::Set { graph, set }) => set::show(ui, project, graph, set, edits),
        Some(ElementId::Legend(g)) => legend::show(ui, project, g, edits),
        Some(ElementId::StringObj(i)) => object::string(ui, project, i, edits),
        Some(ElementId::LineObj(i)) => object::line(ui, project, i, edits),
        Some(ElementId::BoxObj(i)) => object::boxlike(ui, project, i, false, edits),
        Some(ElementId::EllipseObj(i)) => object::boxlike(ui, project, i, true, edits),
        Some(ElementId::Timestamp) => object::timestamp(ui, project, edits),
    });
}

/// The element's parent in the model hierarchy (breadcrumb ancestry).
/// Top-level elements (graphs, annotations, the timestamp) belong to the
/// page itself.
fn parent(id: ElementId) -> Option<ElementId> {
    use ElementId::*;
    match id {
        Graph(_) | StringObj(_) | LineObj(_) | BoxObj(_) | EllipseObj(_) | Timestamp => None,
        Frame(g) | Title(g) | Subtitle(g) | Legend(g) => Some(Graph(g)),
        AxisBar { graph, .. } | Set { graph, .. } => Some(Graph(graph)),
        TickLabels { graph, axis } | AxisLabel { graph, axis } => Some(AxisBar { graph, axis }),
    }
}

/// Short breadcrumb segment name — the ancestry spells the context, so
/// segments don't repeat it (compare `tree::describe`).
fn segment_name(project: &Project, id: ElementId) -> String {
    use ElementId::*;
    match id {
        Graph(g) => format!("Graph {g}"),
        Frame(_) => "Frame".into(),
        Title(_) => "Title".into(),
        Subtitle(_) => "Subtitle".into(),
        Legend(_) => "Legend".into(),
        AxisBar { axis, .. } => crate::tree::AXIS_NAMES[axis.min(3)].into(),
        TickLabels { .. } => "Tick labels".into(),
        AxisLabel { .. } => "Label".into(),
        Set { graph, set } => {
            let legend = project.graphs.get(graph).and_then(|gr| {
                let s = gr.sets.get(set)?;
                (!s.legend.is_empty())
                    .then(|| crate::tree::plain(project, gr.legend.font, &s.legend))
            });
            match legend {
                None => format!("S{set}"),
                Some(l) => format!("S{set} — {}", crate::tree::truncate(&l, 20)),
            }
        }
        StringObj(i) => format!("String {i}"),
        LineObj(i) => format!("Line {i}"),
        BoxObj(i) => format!("Box {i}"),
        EllipseObj(i) => format!("Ellipse {i}"),
        Timestamp => "Timestamp".into(),
    }
}

/// `Page › Graph 0 › X axis › Tick labels`: ancestor segments are links
/// that select that element ("Page" clears the selection); the last segment
/// is the selected element itself.
fn breadcrumb(ui: &mut egui::Ui, project: &Project, selection: &mut Option<ElementId>) {
    let mut chain = Vec::new();
    let mut cur = *selection;
    while let Some(id) = cur {
        chain.push(id);
        cur = parent(id);
    }
    chain.reverse();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if chain.is_empty() {
            ui.strong("Page");
        } else if ui.link("Page").clicked() {
            *selection = None;
        }
        for (i, id) in chain.iter().enumerate() {
            ui.weak("›");
            let name = segment_name(project, *id);
            if i + 1 == chain.len() {
                ui.strong(name);
            } else if ui.link(name).clicked() {
                *selection = Some(*id);
            }
        }
    });
}
