//! Application state and the top-level panel layout.

use std::path::PathBuf;

use oxygrace::{ElementId, FontSet, Project, RenderInfo};

use crate::edit::Edit;
use crate::undo::UndoStack;

pub struct App {
    /// Embedded fonts, loaded once for the app's lifetime.
    pub fonts: FontSet,
    pub project: Option<Project>,
    pub path: Option<PathBuf>,
    /// Model changed → re-render the plot texture on the next frame.
    pub dirty: bool,
    pub texture: Option<egui::TextureHandle>,
    /// Page size of the last render, in device pixels.
    pub page_size: (u32, u32),
    /// Element geometry of the last render (hit-testing).
    pub render_info: Option<RenderInfo>,
    pub selection: Option<ElementId>,
    /// Set by any selecting click (tree or canvas): the inspector then
    /// re-focuses its sections onto the clicked (sub-)element.
    pub refocus: bool,
    /// Device position of the last canvas click — clicking the same spot
    /// again cycles through overlapping elements.
    pub last_click: Option<(f32, f32)>,
    /// Element under the pointer this frame (hover highlight).
    pub hover: Option<ElementId>,
    /// In-flight canvas drag (move element / resize viewport).
    pub drag: Option<crate::plot_view::DragState>,
    /// Element armed for rotation (second click on a rotatable selection).
    pub rotate_armed: Option<ElementId>,
    /// Theme preference (View → Mode); System follows the OS dark/light.
    pub theme_pref: crate::theme::Pref,
    /// Currently applied concrete mode (resolved from the preference).
    pub theme: crate::theme::Mode,
    /// Free page aspect: the page follows the canvas size (xmgrace -free).
    pub free_aspect: bool,
    /// Page size to restore when free aspect is switched off.
    saved_page: Option<(u32, u32)>,
    /// Last seen canvas size in points (drives free-aspect page sizing).
    pub canvas_size: (u32, u32),
    /// One-line status message shown at the bottom.
    pub status: String,
    pub undo: UndoStack,
    /// Unsaved edits exist.
    pub modified: bool,
    /// True while the previous edit was part of a continuous gesture
    /// (slider drag / typing) — same-label live edits then share one
    /// undo snapshot.
    coalescing: bool,
    /// Unsaved-changes dialog is showing.
    confirm_close: bool,
    /// User chose to discard changes; let the next close through.
    allow_close: bool,
    /// Last window title sent to the viewport (avoids resending every frame).
    title_sent: String,
    /// Frame counter for the `OXYGRACE_GUI_SHOT` self-screenshot mode.
    frames: u32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply(&cc.egui_ctx, crate::theme::Mode::Dark);
        let mut app = App {
            fonts: FontSet::load(),
            project: None,
            path: None,
            dirty: false,
            texture: None,
            page_size: (0, 0),
            render_info: None,
            selection: None,
            refocus: false,
            last_click: None,
            hover: None,
            drag: None,
            rotate_armed: None,
            theme_pref: crate::theme::Pref::System,
            theme: crate::theme::Mode::Dark,
            free_aspect: false,
            saved_page: None,
            canvas_size: (0, 0),
            status: "Open a .agr file to begin (Ctrl+O)".into(),
            undo: UndoStack::default(),
            modified: false,
            coalescing: false,
            confirm_close: false,
            allow_close: false,
            title_sent: String::new(),
            frames: 0,
        };
        // Command line: project / data files plus xmgrace-style options.
        let launch = crate::args::parse(std::env::args().skip(1));
        if let Some(project) = launch.project {
            app.project = Some(project);
            app.path = launch.path;
            app.dirty = true;
            app.status = if launch.messages.is_empty() {
                "Loaded".into()
            } else {
                launch.messages.join("; ")
            };
        } else if !launch.messages.is_empty() {
            app.status = launch.messages.join("; ");
        }
        app.free_aspect = launch.free_aspect;
        // Debug/CI hook: pre-select an element (e.g. "set:0:1", "legend:0").
        if let Ok(spec) = std::env::var("OXYGRACE_GUI_SELECT") {
            app.selection = parse_element_spec(&spec);
            app.refocus = true;
        }
        // Debug/CI hook: force a mode.
        match std::env::var("OXYGRACE_GUI_THEME").as_deref() {
            Ok("light") => app.theme_pref = crate::theme::Pref::Light,
            Ok("dark") => app.theme_pref = crate::theme::Pref::Dark,
            _ => {}
        }
        app
    }

    /// Apply one queued edit: snapshot for undo (unless it coalesces into
    /// the gesture in flight), mutate, mark dirty.
    pub fn apply_edit(&mut self, e: Edit) {
        let Some(project) = &mut self.project else { return };
        let coalesce =
            e.coalesce && self.coalescing && self.undo.undo_label() == Some(e.label);
        if !coalesce {
            self.undo.push(project.clone(), e.label);
        }
        self.coalescing = e.coalesce;
        (e.apply)(project);
        self.dirty = true;
        self.modified = true;
    }

    /// End the current coalescing gesture: the next same-label edit starts
    /// a fresh undo step (called when a canvas drag is released).
    pub fn end_gesture(&mut self) {
        self.coalescing = false;
    }

    fn undo_action(&mut self) {
        if let Some(project) = &mut self.project {
            if let Some(label) = self.undo.undo(project) {
                self.status = format!("Undid: {label}");
                self.dirty = true;
                self.modified = true;
                self.coalescing = false;
            }
        }
    }

    fn redo_action(&mut self) {
        if let Some(project) = &mut self.project {
            if let Some(label) = self.undo.redo(project) {
                self.status = format!("Redid: {label}");
                self.dirty = true;
                self.modified = true;
                self.coalescing = false;
            }
        }
    }

    /// Keep the window title in sync: `name.agr * — Oxygrace`.
    fn sync_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".into());
        let title = format!("{name}{} — Oxygrace", if self.modified { " *" } else { "" });
        if title != self.title_sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_sent = title;
        }
    }

    /// Unsaved-changes dialog (shown after an intercepted close request).
    fn confirm_close_modal(&mut self, ctx: &egui::Context) {
        if !self.confirm_close {
            return;
        }
        egui::Modal::new(egui::Id::new("confirm_close")).show(ctx, |ui| {
            ui.heading("Unsaved changes");
            ui.label("The project has unsaved changes.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save and quit").clicked() {
                    crate::file::save(self);
                    if !self.modified {
                        self.allow_close = true;
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    self.confirm_close = false;
                }
                if ui.button("Discard and quit").clicked() {
                    self.allow_close = true;
                    self.confirm_close = false;
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_close = false;
                }
            });
        });
    }

    /// Debug/CI hook: with `OXYGRACE_GUI_SHOT=<path.png>` set, capture the
    /// window a few frames after startup, write it to the path and quit.
    fn self_screenshot(&mut self, ctx: &egui::Context) {
        let Ok(path) = std::env::var("OXYGRACE_GUI_SHOT") else {
            return;
        };
        self.frames += 1;
        ctx.request_repaint(); // keep frames flowing while we wait
        if self.frames == 10 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            let mut pixmap =
                tiny_skia::Pixmap::new(image.size[0] as u32, image.size[1] as u32).unwrap();
            pixmap.data_mut().copy_from_slice(&rgba);
            if let Err(e) = std::fs::write(&path, pixmap.encode_png().unwrap()) {
                log::error!("screenshot write failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // Collect bar-button responses so an open menu follows hover.
                let mut bar_buttons: Vec<egui::Response> = Vec::new();
                let (resp, _) = egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
                    if ui.button("Open…").clicked() {
                        crate::file::open_dialog(self);
                    }
                    ui.separator();
                    let has_project = self.project.is_some();
                    if ui
                        .add_enabled(has_project, egui::Button::new("Save"))
                        .clicked()
                    {
                        crate::file::save(self);
                    }
                    if ui
                        .add_enabled(has_project, egui::Button::new("Save As…"))
                        .clicked()
                    {
                        crate::file::save_as(self);
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                bar_buttons.push(resp);
                let (resp, _) = egui::containers::menu::MenuButton::new("Edit").ui(ui, |ui| {
                    let undo_text = match self.undo.undo_label() {
                        Some(l) => format!("Undo {l}"),
                        None => "Undo".into(),
                    };
                    if ui
                        .add_enabled(self.undo.undo_label().is_some(), egui::Button::new(undo_text))
                        .clicked()
                    {
                        self.undo_action();
                    }
                    let redo_text = match self.undo.redo_label() {
                        Some(l) => format!("Redo {l}"),
                        None => "Redo".into(),
                    };
                    if ui
                        .add_enabled(self.undo.redo_label().is_some(), egui::Button::new(redo_text))
                        .clicked()
                    {
                        self.redo_action();
                    }
                });
                bar_buttons.push(resp);
                let (resp, _) = egui::containers::menu::MenuButton::new("View").ui(ui, |ui| {
                    if ui
                        .checkbox(&mut self.free_aspect, "Free aspect")
                        .on_hover_text("Page follows the window size (xmgrace -free)")
                        .changed()
                    {
                        self.toggle_free_aspect();
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Mode").weak());
                    ui.radio_value(&mut self.theme_pref, crate::theme::Pref::System, "System");
                    ui.radio_value(&mut self.theme_pref, crate::theme::Pref::Dark, "Dark");
                    ui.radio_value(&mut self.theme_pref, crate::theme::Pref::Light, "Light");
                });
                bar_buttons.push(resp);

                // Open menus follow hover across the bar (one was clicked →
                // hovering a sibling switches to it).
                let ctx = ui.ctx().clone();
                let ids: Vec<egui::Id> = bar_buttons
                    .iter()
                    .map(egui::Popup::default_response_id)
                    .collect();
                if ids.iter().any(|id| egui::Popup::is_id_open(&ctx, *id)) {
                    for (resp, id) in bar_buttons.iter().zip(&ids) {
                        if resp.hovered() && !egui::Popup::is_id_open(&ctx, *id) {
                            egui::Popup::open_id(&ctx, *id);
                        }
                    }
                }
            });
        });
    }

    /// Toggle free page aspect, remembering / restoring the project's own
    /// page geometry (viewports scale back along with the page).
    fn toggle_free_aspect(&mut self) {
        if let Some(p) = &mut self.project {
            if self.free_aspect {
                self.saved_page = Some((p.page_width, p.page_height));
            } else if let Some((w, h)) = self.saved_page.take() {
                rescale_views(p, (w, h));
                p.page_width = w;
                p.page_height = h;
                self.dirty = true;
            }
        }
    }
}

/// Rescale every viewport and view-anchored object from the current page
/// extents to the target page's — the same proportional stretch the reader
/// applies to old-format files (`postprocess_version`, after graphs.cpp
/// `postprocess_project`), so the plot keeps filling the same fraction of
/// the page when its aspect changes.
fn rescale_views(p: &mut Project, to: (u32, u32)) {
    let (fw, fh) = (p.page_width as f64, p.page_height as f64);
    let fside = fw.min(fh);
    let (tw, th) = (to.0 as f64, to.1 as f64);
    let tside = tw.min(th);
    let ex = (tw / tside) / (fw / fside);
    let ey = (th / tside) / (fh / fside);
    if !(ex.is_finite() && ey.is_finite()) {
        return;
    }
    for g in &mut p.graphs {
        g.view.xmin *= ex;
        g.view.xmax *= ex;
        g.view.ymin *= ey;
        g.view.ymax *= ey;
        if g.legend.loctype_view {
            g.legend.x *= ex;
            g.legend.y *= ey;
        }
    }
    for s in &mut p.strings {
        if s.loctype_view {
            s.x *= ex;
            s.y *= ey;
        }
    }
    for l in &mut p.lines {
        if l.loctype_view {
            l.x1 *= ex;
            l.y1 *= ey;
            l.x2 *= ex;
            l.y2 *= ey;
        }
    }
    for b in p.boxes.iter_mut().chain(p.ellipses.iter_mut()) {
        if b.loctype_view {
            b.x1 *= ex;
            b.y1 *= ey;
            b.x2 *= ex;
            b.y2 *= ey;
        }
    }
    p.timestamp.x *= ex;
    p.timestamp.y *= ey;
}

/// Parse `OXYGRACE_GUI_SELECT` specs like `graph:0`, `set:0:1`, `axis:0:0`,
/// `legend:0`, `title:0`, `string:2`, `timestamp` (debug/CI hook).
fn parse_element_spec(spec: &str) -> Option<ElementId> {
    let parts: Vec<&str> = spec.split(':').collect();
    let idx = |i: usize| parts.get(i).and_then(|s| s.parse::<usize>().ok());
    Some(match (parts.first().copied()?, idx(1), idx(2)) {
        ("graph", Some(g), _) => ElementId::Graph(g),
        ("frame", Some(g), _) => ElementId::Frame(g),
        ("title", Some(g), _) => ElementId::Title(g),
        ("subtitle", Some(g), _) => ElementId::Subtitle(g),
        ("axis", Some(g), Some(a)) => ElementId::AxisBar { graph: g, axis: a },
        ("ticklabels", Some(g), Some(a)) => ElementId::TickLabels { graph: g, axis: a },
        ("axislabel", Some(g), Some(a)) => ElementId::AxisLabel { graph: g, axis: a },
        ("set", Some(g), Some(s)) => ElementId::Set { graph: g, set: s },
        ("legend", Some(g), _) => ElementId::Legend(g),
        ("string", Some(i), _) => ElementId::StringObj(i),
        ("line", Some(i), _) => ElementId::LineObj(i),
        ("box", Some(i), _) => ElementId::BoxObj(i),
        ("ellipse", Some(i), _) => ElementId::EllipseObj(i),
        ("timestamp", _, _) => ElementId::Timestamp,
        _ => return None,
    })
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply the theme when the preference or the OS mode changes
        // (App::new already applied the initial Dark).
        let resolved = crate::theme::resolve(ctx, self.theme_pref);
        if resolved != self.theme {
            self.theme = resolved;
            crate::theme::apply(ctx, resolved);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O)) {
            crate::file::open_dialog(self);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            crate::file::save(self);
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::Y)
        }) {
            self.redo_action();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Z)) {
            self.undo_action();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection = None;
            self.rotate_armed = None;
        }
        // Intercept window close while there are unsaved changes.
        if ctx.input(|i| i.viewport().close_requested()) && self.modified && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        // Free aspect: the page follows the canvas size AND the viewports
        // scale with the page's view extents, so the plot stretches to fill
        // the window (not an undoable edit — the project's own geometry is
        // restored when toggled off).
        if self.free_aspect {
            let (w, h) = self.canvas_size;
            if w > 50 && h > 50 {
                if let Some(p) = &mut self.project {
                    if p.page_width != w || p.page_height != h {
                        rescale_views(p, (w, h));
                        p.page_width = w;
                        p.page_height = h;
                        self.dirty = true;
                    }
                }
            }
        }
        crate::render::refresh_texture(self, ctx);
        self.sync_title(ctx);
        self.self_screenshot(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.menu_bar(ui);
        // Note: egui persists panel widths by id — bump the id suffix when
        // changing the size policy, or stale stored widths win.
        egui::Panel::left("tree_v2")
            .resizable(true)
            .default_size(220.0)
            .size_range(140.0..=380.0)
            .show_inside(ui, |ui| {
                ui.heading("Project");
                ui.separator();
                match &self.project {
                    Some(project) => {
                        if crate::tree::show(ui, project, &mut self.selection) {
                            self.refocus = true;
                            self.rotate_armed = None;
                        }
                    }
                    None => {
                        ui.weak("No file open");
                    }
                }
            });
        let focus_changed = std::mem::take(&mut self.refocus);
        let mut edits: Vec<Edit> = Vec::new();
        egui::Panel::right("inspector_v2")
            .resizable(true)
            .default_size(310.0)
            .size_range(200.0..=400.0)
            .show_inside(ui, |ui| {
                ui.heading("Properties");
                ui.separator();
                if let Some(project) = &self.project {
                    crate::inspector::show(ui, project, self.selection, focus_changed, &mut edits);
                } else {
                    ui.weak("No file open");
                }
            });
        for e in edits {
            self.apply_edit(e);
        }
        self.confirm_close_modal(ui.ctx());
        egui::Panel::bottom("status").show_inside(ui, |ui| {
            ui.label(&self.status);
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::theme::canvas_bg(self.theme)))
            .show_inside(ui, |ui| {
                crate::plot_view::show(self, ui);
            });
    }
}
