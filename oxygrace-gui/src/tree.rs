//! The project tree (left panel): every selectable element of the model,
//! sharing the same `ElementId` selection currency as the plot canvas and
//! the inspector, so clicking any side highlights all of them. Rows for
//! empty/disabled elements (title, axis label, …) stay present but dimmed —
//! the inspector pages are element-exact, so the tree is how you reach a
//! not-yet-drawn element to fill it in.

use oxygrace::{ElementId, Project};

pub(crate) const AXIS_NAMES: [&str; 4] = ["X axis", "Y axis", "Alt X axis", "Alt Y axis"];

/// Returns true when a row was clicked (the app then resets click-cycling
/// state such as the armed rotation).
///
/// `reveal` is true for a short window of frames after the selection
/// changed (canvas click, breadcrumb): collapsed ancestor nodes of the
/// selected element expand and the tree scrolls the selected row into
/// view. It stays true for several frames — not just one — because egui's
/// expand and scroll animations are wall-clock based, so the row's final
/// position only exists once they settle.
pub fn show(
    ui: &mut egui::Ui,
    project: &Project,
    selection: &mut Option<ElementId>,
    reveal: bool,
) -> bool {
    use ElementId::*;
    let mut clicked = false;
    let clicked = &mut clicked;
    let sel = *selection;
    // `Some(true)` force-opens a group this frame, `None` leaves it alone.
    let open_if = |cond: bool| (reveal && cond).then_some(true);
    // Scroll both ways: long labels scroll horizontally instead of setting
    // a minimum panel width, so the panel can be made arbitrarily narrow.
    // No auto-shrink: the area must span the full panel even when the
    // content is smaller, so the scrollbar sits at the panel edge.
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        // The page itself — selected whenever nothing else is, matching the
        // breadcrumb's "Page" root (Esc / empty-canvas click land here).
        if ui.selectable_label(selection.is_none(), "Page").clicked() {
            *selection = None;
            *clicked = true;
        }
        for (g, graph) in project.graphs.iter().enumerate() {
            let title = if graph.labels.title.is_empty() {
                format!("Graph {g}")
            } else {
                let text = plain(project, graph.labels.title_font, &graph.labels.title);
                format!("Graph {g} — {}", truncate(&text, 24))
            };
            // Selectable header (the graph itself — same name the breadcrumb
            // uses), with the graph's parts as children.
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                ui.make_persistent_id(("graph_node", g)),
                project.graphs.len() <= 3,
            );
            if reveal && owning_graph(sel) == Some(g) {
                state.set_open(true);
            }
            state
                .show_header(ui, |ui| {
                    row(ui, selection, clicked, ElementId::Graph(g), &title, graph.hidden, reveal);
                })
                .body(|ui| {
                    row(ui, selection, clicked, ElementId::Frame(g), "Frame", false, reveal);
                    row(
                        ui,
                        selection,
                        clicked,
                        ElementId::Title(g),
                        "Title",
                        graph.labels.title.is_empty(),
                        reveal,
                    );
                    row(
                        ui,
                        selection,
                        clicked,
                        ElementId::Subtitle(g),
                        "Subtitle",
                        graph.labels.subtitle.is_empty(),
                        reveal,
                    );
                    row(
                        ui,
                        selection,
                        clicked,
                        ElementId::Legend(g),
                        "Legend",
                        !graph.legend.active,
                        reveal,
                    );
                    let in_axes = matches!(sel,
                        Some(AxisBar { graph, .. } | TickLabels { graph, .. }
                            | AxisLabel { graph, .. }) if graph == g);
                    egui::CollapsingHeader::new("Axes")
                        .id_salt(("axes", g))
                        .default_open(true)
                        .open(open_if(in_axes))
                        .show(ui, |ui| {
                            for (a, name) in AXIS_NAMES.iter().enumerate() {
                                axis_node(ui, selection, clicked, graph, g, a, name, reveal);
                            }
                        });
                    let in_sets = matches!(sel, Some(Set { graph, .. }) if graph == g);
                    egui::CollapsingHeader::new("Sets")
                        .id_salt(("sets", g))
                        .default_open(true)
                        .open(open_if(in_sets))
                        .show(ui, |ui| {
                            for (s, set) in graph.sets.iter().enumerate() {
                                let label = if set.legend.is_empty() {
                                    format!("S{s} ({:?})", set.set_type)
                                } else {
                                    let text =
                                        plain(project, graph.legend.font, &set.legend);
                                    format!("S{s} — {}", truncate(&text, 20))
                                };
                                row(
                                    ui,
                                    selection,
                                    clicked,
                                    ElementId::Set { graph: g, set: s },
                                    &label,
                                    set.hidden,
                                    reveal,
                                );
                            }
                        });
                });
        }

        let nobjs =
            project.strings.len() + project.lines.len() + project.boxes.len() + project.ellipses.len();
        if nobjs > 0 {
            let in_objs = matches!(
                sel,
                Some(StringObj(_) | LineObj(_) | BoxObj(_) | EllipseObj(_))
            );
            egui::CollapsingHeader::new("Annotations")
                .default_open(nobjs <= 12)
                .open(open_if(in_objs))
                .show(ui, |ui| {
                    for (i, s) in project.strings.iter().enumerate() {
                        let text = plain(project, s.font, &s.text);
                        let label = format!("String {i} — {}", truncate(&text, 20));
                        row(ui, selection, clicked, ElementId::StringObj(i), &label, !s.active, reveal);
                    }
                    for (i, l) in project.lines.iter().enumerate() {
                        row(ui, selection, clicked, ElementId::LineObj(i), &format!("Line {i}"), !l.active, reveal);
                    }
                    for (i, b) in project.boxes.iter().enumerate() {
                        row(ui, selection, clicked, ElementId::BoxObj(i), &format!("Box {i}"), !b.active, reveal);
                    }
                    for (i, e) in project.ellipses.iter().enumerate() {
                        row(
                            ui,
                            selection,
                            clicked,
                            ElementId::EllipseObj(i),
                            &format!("Ellipse {i}"),
                            !e.active,
                            reveal,
                        );
                    }
                });
        }
        if project.timestamp.active {
            row(ui, selection, clicked, ElementId::Timestamp, "Timestamp", false, reveal);
        }
    });
    *clicked
}

/// Graph whose subtree holds a selected element (None for the graph row
/// itself — visible without expanding — and for page-level elements).
fn owning_graph(sel: Option<ElementId>) -> Option<usize> {
    use ElementId::*;
    match sel? {
        Frame(g) | Title(g) | Subtitle(g) | Legend(g) => Some(g),
        AxisBar { graph, .. }
        | TickLabels { graph, .. }
        | AxisLabel { graph, .. }
        | Set { graph, .. } => Some(graph),
        _ => None,
    }
}

/// One axis in the tree: a selectable header (the axis itself) with its
/// tick-labels and label sub-elements as children — the same granularity
/// the inspector pages and canvas hit-testing use.
#[allow(clippy::too_many_arguments)]
fn axis_node(
    ui: &mut egui::Ui,
    selection: &mut Option<ElementId>,
    clicked: &mut bool,
    graph: &oxygrace::model::Graph,
    g: usize,
    a: usize,
    name: &str,
    reveal: bool,
) {
    let axis = &graph.axes[a];
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id(("axis_node", g, a)),
        false,
    );
    let has_child = matches!(*selection,
        Some(ElementId::TickLabels { graph, axis } | ElementId::AxisLabel { graph, axis })
            if graph == g && axis == a);
    if reveal && has_child {
        state.set_open(true);
    }
    state
    .show_header(ui, |ui| {
        row(ui, selection, clicked, ElementId::AxisBar { graph: g, axis: a }, name, !axis.active, reveal);
    })
    .body(|ui| {
        row(
            ui,
            selection,
            clicked,
            ElementId::TickLabels { graph: g, axis: a },
            "Tick labels",
            !axis.active || !axis.ticklabels,
            reveal,
        );
        row(
            ui,
            selection,
            clicked,
            ElementId::AxisLabel { graph: g, axis: a },
            "Label",
            !axis.active || axis.label.is_empty(),
            reveal,
        );
    });
}

/// One selectable tree row; `dim` grays out hidden/inactive elements. On
/// the reveal frame (selection freshly changed elsewhere) the selected row
/// scrolls into view if it is outside the visible tree.
fn row(
    ui: &mut egui::Ui,
    selection: &mut Option<ElementId>,
    clicked: &mut bool,
    id: ElementId,
    text: &str,
    dim: bool,
    reveal: bool,
) {
    let selected = *selection == Some(id);
    let text = match (dim, selected) {
        // A *selected* row takes its ink from the selection plate
        // (`visuals.selection.stroke`, which is what egui uses for a selected
        // widget's text). `weak()` sets an explicit color and so wins over
        // that — dim panel grey on the plate, i.e. grey on grey. Dim it
        // *against the plate* instead, so "element is empty" still reads.
        (true, true) => egui::RichText::new(text)
            .color(ui.visuals().selection.stroke.color.gamma_multiply(0.75)),
        (true, false) => egui::RichText::new(text).weak(),
        (false, _) => egui::RichText::new(text),
    };
    let resp = ui.selectable_label(selected, text);
    if reveal && selected {
        // Scroll to the row's *left edge* only: targeting the full rect
        // would also scroll horizontally to fit long labels, shoving the
        // whole tree sideways.
        let edge = egui::Rect::from_min_size(resp.rect.min, egui::vec2(1.0, resp.rect.height()));
        ui.scroll_to_rect(edge, None);
    }
    if resp.clicked() {
        *selection = Some(id);
        *clicked = true;
    }
}

/// Grace markup flattened for a plain-text label (Greek etc. shown as the
/// characters they render as, escape codes dropped).
pub(crate) fn plain(project: &Project, font: i32, s: &str) -> String {
    oxygrace::text::plain(s, font, &project.font_map)
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
            let legend = project.and_then(|p| {
                let gr = p.graphs.get(graph)?;
                let s = gr.sets.get(set)?;
                (!s.legend.is_empty()).then(|| plain(p, gr.legend.font, &s.legend))
            });
            match legend {
                None => format!("Set G{graph}.S{set}"),
                Some(l) => format!("Set G{graph}.S{set} — {}", truncate(&l, 30)),
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

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
