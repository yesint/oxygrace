//! The project tree (left panel): every selectable element of the model,
//! sharing the same `ElementId` selection currency as the plot canvas, so
//! clicking either side highlights both.

use oxygrace::{ElementId, Project};

const AXIS_NAMES: [&str; 4] = ["X axis", "Y axis", "Alt X axis", "Alt Y axis"];

/// Returns true when a row was clicked (the inspector then re-focuses its
/// sections onto the clicked element, even if it was already selected).
pub fn show(ui: &mut egui::Ui, project: &Project, selection: &mut Option<ElementId>) -> bool {
    let mut clicked = false;
    let clicked = &mut clicked;
    // Scroll both ways: long labels scroll horizontally instead of setting
    // a minimum panel width, so the panel can be made arbitrarily narrow.
    egui::ScrollArea::both().show(ui, |ui| {
        for (g, graph) in project.graphs.iter().enumerate() {
            let title = if graph.labels.title.is_empty() {
                format!("Graph {g}")
            } else {
                format!("Graph {g} — {}", truncate(&graph.labels.title, 24))
            };
            let header = if graph.hidden {
                egui::RichText::new(title).weak()
            } else {
                egui::RichText::new(title)
            };
            egui::CollapsingHeader::new(header)
                .id_salt(("graph", g))
                .default_open(project.graphs.len() <= 3)
                .show(ui, |ui| {
                    row(ui, selection, clicked, ElementId::Graph(g), "Plot area", graph.hidden);
                    if !graph.labels.title.is_empty() {
                        row(ui, selection, clicked, ElementId::Title(g), "Title", false);
                    }
                    if !graph.labels.subtitle.is_empty() {
                        row(ui, selection, clicked, ElementId::Subtitle(g), "Subtitle", false);
                    }
                    row(
                        ui,
                        selection,
                        clicked,
                        ElementId::Legend(g),
                        "Legend",
                        !graph.legend.active,
                    );
                    egui::CollapsingHeader::new("Axes")
                        .id_salt(("axes", g))
                        .default_open(true)
                        .show(ui, |ui| {
                            for (a, name) in AXIS_NAMES.iter().enumerate() {
                                row(
                                    ui,
                                    selection,
                                    clicked,
                                    ElementId::AxisBar { graph: g, axis: a },
                                    name,
                                    !graph.axes[a].active,
                                );
                            }
                        });
                    egui::CollapsingHeader::new("Sets")
                        .id_salt(("sets", g))
                        .default_open(true)
                        .show(ui, |ui| {
                            for (s, set) in graph.sets.iter().enumerate() {
                                let label = if set.legend.is_empty() {
                                    format!("S{s} ({:?})", set.set_type)
                                } else {
                                    format!("S{s} — {}", truncate(&set.legend, 20))
                                };
                                row(
                                    ui,
                                    selection,
                                    clicked,
                                    ElementId::Set { graph: g, set: s },
                                    &label,
                                    set.hidden,
                                );
                            }
                        });
                });
        }

        let nobjs =
            project.strings.len() + project.lines.len() + project.boxes.len() + project.ellipses.len();
        if nobjs > 0 {
            egui::CollapsingHeader::new("Annotations")
                .default_open(nobjs <= 12)
                .show(ui, |ui| {
                    for (i, s) in project.strings.iter().enumerate() {
                        let label = format!("String {i} — {}", truncate(&s.text, 20));
                        row(ui, selection, clicked, ElementId::StringObj(i), &label, !s.active);
                    }
                    for (i, l) in project.lines.iter().enumerate() {
                        row(ui, selection, clicked, ElementId::LineObj(i), &format!("Line {i}"), !l.active);
                    }
                    for (i, b) in project.boxes.iter().enumerate() {
                        row(ui, selection, clicked, ElementId::BoxObj(i), &format!("Box {i}"), !b.active);
                    }
                    for (i, e) in project.ellipses.iter().enumerate() {
                        row(
                            ui,
                            selection,
                            clicked,
                            ElementId::EllipseObj(i),
                            &format!("Ellipse {i}"),
                            !e.active,
                        );
                    }
                });
        }
        if project.timestamp.active {
            row(ui, selection, clicked, ElementId::Timestamp, "Timestamp", false);
        }
    });
    *clicked
}

/// One selectable tree row; `dim` grays out hidden/inactive elements.
fn row(
    ui: &mut egui::Ui,
    selection: &mut Option<ElementId>,
    clicked: &mut bool,
    id: ElementId,
    text: &str,
    dim: bool,
) {
    let text = if dim {
        egui::RichText::new(text).weak()
    } else {
        egui::RichText::new(text)
    };
    if ui.selectable_label(*selection == Some(id), text).clicked() {
        *selection = Some(id);
        *clicked = true;
    }
}

/// Human-readable description of an element (status bar / inspector header).
pub fn describe(project: Option<&Project>, id: ElementId) -> String {
    use ElementId::*;
    match id {
        Graph(g) => format!("Graph {g}"),
        Frame(g) => format!("Frame of graph {g}"),
        Title(g) => format!("Title of graph {g}"),
        Subtitle(g) => format!("Subtitle of graph {g}"),
        AxisBar { graph, axis } => format!("{} of graph {graph}", AXIS_NAMES[axis.min(3)]),
        TickLabels { graph, axis } => {
            format!("{} tick labels of graph {graph}", AXIS_NAMES[axis.min(3)])
        }
        AxisLabel { graph, axis } => {
            format!("{} label of graph {graph}", AXIS_NAMES[axis.min(3)])
        }
        Set { graph, set } => {
            let legend = project
                .and_then(|p| p.graphs.get(graph))
                .and_then(|g| g.sets.get(set))
                .map(|s| s.legend.as_str())
                .unwrap_or("");
            if legend.is_empty() {
                format!("Set G{graph}.S{set}")
            } else {
                format!("Set G{graph}.S{set} — {}", truncate(legend, 30))
            }
        }
        Legend(g) => format!("Legend of graph {g}"),
        StringObj(i) => format!("String annotation {i}"),
        LineObj(i) => format!("Line annotation {i}"),
        BoxObj(i) => format!("Box annotation {i}"),
        EllipseObj(i) => format!("Ellipse annotation {i}"),
        Timestamp => "Timestamp".into(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
