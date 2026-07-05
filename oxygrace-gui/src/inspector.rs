//! The property inspector (right panel): dispatches the current selection
//! to a per-element page. All pages are built from the shared row
//! vocabulary in [`rows`], so they look and behave identically; every page
//! follows the same section rhythm (visibility → geometry → line/pen →
//! fill → text).

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

/// `focus_changed` is true on the frame the selection changed: pages then
/// force-expand the section matching the clicked sub-element and fold the
/// rest (afterwards the user can open/close sections freely).
pub fn show(
    ui: &mut egui::Ui,
    project: &Project,
    selection: Option<ElementId>,
    focus_changed: bool,
    edits: &mut Vec<Edit>,
) {
    let Some(id) = selection else {
        page::show(ui, project, edits);
        return;
    };
    // Sub-elements (title, tick labels, …) share their parent's property
    // page — the header names the page, not the clicked fragment.
    let header = match id {
        ElementId::Title(g) | ElementId::Subtitle(g) | ElementId::Frame(g) => ElementId::Graph(g),
        ElementId::TickLabels { graph, axis } | ElementId::AxisLabel { graph, axis } => {
            ElementId::AxisBar { graph, axis }
        }
        other => other,
    };
    ui.label(crate::tree::describe(Some(project), header));
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| match id {
        ElementId::Graph(g) => {
            graph::show(ui, project, g, graph::Focus::Area, focus_changed, edits)
        }
        ElementId::Title(g) => {
            graph::show(ui, project, g, graph::Focus::Title, focus_changed, edits)
        }
        ElementId::Subtitle(g) => {
            graph::show(ui, project, g, graph::Focus::Subtitle, focus_changed, edits)
        }
        ElementId::Frame(g) => {
            graph::show(ui, project, g, graph::Focus::Frame, focus_changed, edits)
        }
        ElementId::AxisBar { graph, axis } => {
            axis::show(ui, project, graph, axis, axis::Focus::Bar, focus_changed, edits)
        }
        ElementId::TickLabels { graph, axis } => {
            axis::show(ui, project, graph, axis, axis::Focus::TickLabels, focus_changed, edits)
        }
        ElementId::AxisLabel { graph, axis } => {
            axis::show(ui, project, graph, axis, axis::Focus::Label, focus_changed, edits)
        }
        ElementId::Set { graph, set } => set::show(ui, project, graph, set, edits),
        ElementId::Legend(g) => legend::show(ui, project, g, edits),
        ElementId::StringObj(i) => object::string(ui, project, i, edits),
        ElementId::LineObj(i) => object::line(ui, project, i, edits),
        ElementId::BoxObj(i) => object::boxlike(ui, project, i, false, edits),
        ElementId::EllipseObj(i) => object::boxlike(ui, project, i, true, edits),
        ElementId::Timestamp => object::timestamp(ui, project, edits),
    });
}
