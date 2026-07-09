//! Application state and the top-level panel layout.

use std::path::PathBuf;

use oxygrace::{ElementId, FontSet, Project, RenderInfo};

use crate::edit::Edit;
use crate::undo::UndoStack;

/// Active canvas tool, switched from the toolbar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Default: select, move and edit plot elements.
    Select,
    /// Drag to pan the world window of the graph under the cursor.
    Pan,
    /// Click a set to autoscale its graph to that set's extents.
    PickSet,
}

/// User preferences (Edit → Settings…), persisted across sessions via
/// eframe storage.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Stack the properties inspector below the project tree in the left
    /// panel instead of using a separate right-side panel.
    pub inspector_below: bool,
}

/// eframe storage key for [`Settings`].
const SETTINGS_KEY: &str = "settings";

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
    /// Selection as of the last frame: when it differs, the tree expands
    /// collapsed ancestors so the newly selected row is visible.
    prev_selection: Option<ElementId>,
    /// Device position of the last canvas click — clicking the same spot
    /// again cycles through overlapping elements.
    pub last_click: Option<(f32, f32)>,
    /// Element under the pointer this frame (hover highlight).
    pub hover: Option<ElementId>,
    /// In-flight canvas drag (move element / resize viewport).
    pub drag: Option<crate::plot_view::DragState>,
    /// Active canvas tool (toolbar): select / pan / autoscale-to-set.
    pub tool: Tool,
    /// In-flight pan gesture.
    pub pan: Option<crate::plot_view::PanState>,
    /// Element armed for rotation (second click on a rotatable selection).
    pub rotate_armed: Option<ElementId>,
    /// Theme preference (View → Mode); System follows the OS dark/light.
    pub theme_pref: crate::theme::Pref,
    /// Currently applied concrete mode (resolved from the preference).
    pub theme: crate::theme::Mode,
    /// Persisted user preferences (Edit → Settings…).
    pub settings: Settings,
    /// The Settings window is showing.
    show_settings: bool,
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
    /// Context handle for async tasks (web file dialogs) to request repaints.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub egui_ctx: egui::Context,
    /// Asynchronously opened files arrive here as (name, content).
    #[cfg(target_arch = "wasm32")]
    pub file_tx: std::sync::mpsc::Sender<(String, String)>,
    #[cfg(target_arch = "wasm32")]
    file_rx: std::sync::mpsc::Receiver<(String, String)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::icons::install(&cc.egui_ctx);
        crate::theme::apply(&cc.egui_ctx, crate::theme::Mode::Dark);
        #[cfg(target_arch = "wasm32")]
        let (file_tx, file_rx) = std::sync::mpsc::channel();
        let mut app = App {
            fonts: FontSet::load(),
            project: None,
            path: None,
            dirty: false,
            texture: None,
            page_size: (0, 0),
            render_info: None,
            selection: None,
            prev_selection: None,
            last_click: None,
            hover: None,
            drag: None,
            tool: Tool::Select,
            pan: None,
            rotate_armed: None,
            theme_pref: crate::theme::Pref::System,
            theme: crate::theme::Mode::Dark,
            settings: cc
                .storage
                .and_then(|s| eframe::get_value(s, SETTINGS_KEY))
                .unwrap_or_default(),
            show_settings: false,
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
            egui_ctx: cc.egui_ctx.clone(),
            #[cfg(target_arch = "wasm32")]
            file_tx,
            #[cfg(target_arch = "wasm32")]
            file_rx,
        };
        // The web demo starts with a bundled example (there is no command
        // line or filesystem in the browser).
        #[cfg(target_arch = "wasm32")]
        {
            app.project = Some(oxygrace::load_str(include_str!(
                "../../examples/log2log.agr"
            )));
            app.path = Some(PathBuf::from("log2log.agr"));
            app.dirty = true;
            app.status = "Demo project loaded — File → Open… to load your own".into();
        }
        // Command line: project / data files plus xmgrace-style options.
        #[cfg(not(target_arch = "wasm32"))]
        {
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
        }
        // Debug/CI hook: pre-select an element (e.g. "set:0:1", "legend:0").
        if let Ok(spec) = std::env::var("OXYGRACE_GUI_SELECT") {
            app.selection = parse_element_spec(&spec);
        }
        // Debug/CI hook: force a mode.
        match std::env::var("OXYGRACE_GUI_THEME").as_deref() {
            Ok("light") => app.theme_pref = crate::theme::Pref::Light,
            Ok("dark") => app.theme_pref = crate::theme::Pref::Dark,
            _ => {}
        }
        // Debug/CI hook: force a panel layout ("stacked" = below the tree).
        if let Ok(v) = std::env::var("OXYGRACE_GUI_LAYOUT") {
            app.settings.inspector_below = v == "stacked";
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
                let (file_resp, _) = egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
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
                let (edit_resp, _) = egui::containers::menu::MenuButton::new("Edit").ui(ui, |ui| {
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
                    ui.separator();
                    if ui.button("Settings…").clicked() {
                        self.show_settings = true;
                    }
                });
                let (view_resp, _) = egui::containers::menu::MenuButton::new("View").ui(ui, |ui| {
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
                menu_hover_follow(ui.ctx(), &[file_resp, edit_resp, view_resp]);
            });
        });
    }

    /// The project-tree panel body (heading + tree). `reveal` expands
    /// collapsed ancestors of a freshly changed selection.
    fn tree_panel(&mut self, ui: &mut egui::Ui, reveal: bool) {
        ui.heading("Project");
        ui.separator();
        match &self.project {
            Some(project) => {
                if crate::tree::show(ui, project, &mut self.selection, reveal) {
                    self.rotate_armed = None;
                }
            }
            None => {
                ui.weak("No file open");
            }
        }
    }

    /// The properties-inspector panel body (heading + breadcrumbed page),
    /// applying whatever edits the page queued.
    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Properties");
        ui.separator();
        let mut edits: Vec<Edit> = Vec::new();
        match &self.project {
            Some(project) => {
                crate::inspector::show(ui, project, &mut self.selection, &mut edits);
            }
            None => {
                ui.weak("No file open");
            }
        }
        for e in edits {
            self.apply_edit(e);
        }
    }

    /// The Settings window (Edit → Settings…); choices persist across
    /// sessions via eframe storage (see [`App::save`]).
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Properties panel").weak());
                ui.radio_value(&mut self.settings.inspector_below, false, "Right of the plot");
                ui.radio_value(&mut self.settings.inspector_below, true, "Below the project tree");
            });
        if !open {
            self.show_settings = false;
        }
    }

    /// Toggle a modal canvas tool from the toolbar (clicking the active
    /// tool returns to Select).
    fn set_tool(&mut self, t: Tool) {
        self.tool = if self.tool == t { Tool::Select } else { t };
        self.pan = None;
    }

    /// Autoscale every graph's world window to its visible sets.
    fn autoscale_all(&mut self) {
        if self.project.is_none() {
            return;
        }
        self.apply_edit(Edit::new("autoscale all", (), false, |p, _: ()| {
            for g in &mut p.graphs {
                oxygrace::import::autoscale_world_filtered(g, |_, s| !s.hidden);
            }
        }));
        self.status = "Autoscaled all graphs to their visible sets".into();
    }

    /// Autoscale `graph` to the extents of one set (the autoscale-to-set
    /// tool), then return to Select.
    pub fn autoscale_to_set(&mut self, graph: usize, set: usize) {
        self.apply_edit(Edit::new(
            "autoscale to set",
            (graph, set),
            false,
            move |p, (g, s)| {
                if let Some(gr) = p.graphs.get_mut(g) {
                    oxygrace::import::autoscale_world_filtered(gr, move |i, _| i == s);
                }
            },
        ));
        self.status = format!("Autoscaled graph {graph} to set S{set}");
        self.tool = Tool::Select;
    }

    /// The toolbar above the project tree: icon-only buttons that wrap to
    /// fill the panel width.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        use crate::icons::{icon_button, Icon};
        let has = self.project.is_some();
        // A toolbar button, disabled while no project is open.
        let btn = |ui: &mut egui::Ui, icon: Icon, tip: &str, active: bool| {
            ui.add_enabled_ui(has, |ui| icon_button(ui, icon, tip, active))
                .inner
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            if icon_button(ui, Icon::Open, "Open… (Ctrl+O)", false).clicked() {
                crate::file::open_dialog(self);
            }
            if btn(ui, Icon::Save, "Save (Ctrl+S)", false).clicked() {
                crate::file::save(self);
            }
            if btn(ui, Icon::AutoscaleAll, "Autoscale all visible sets", false).clicked() {
                self.autoscale_all();
            }
            let pick = self.tool == Tool::PickSet;
            if btn(ui, Icon::AutoscaleSet, "Autoscale to a set — then click a set", pick).clicked()
            {
                self.set_tool(Tool::PickSet);
            }
            let pan = self.tool == Tool::Pan;
            if btn(ui, Icon::Pan, "Pan — drag to move the view", pan).clicked() {
                self.set_tool(Tool::Pan);
            }
            let free = self.free_aspect;
            if btn(ui, Icon::FreeAspect, "Free aspect (page fills the window)", free).clicked() {
                self.free_aspect = !self.free_aspect;
                self.toggle_free_aspect();
            }
        });
    }

    /// Toggle free page aspect, remembering / restoring the project's own
    /// page geometry (viewports scale back along with the page).
    fn toggle_free_aspect(&mut self) {
        if let Some(p) = &mut self.project {
            if self.free_aspect {
                if self.saved_page.is_none() {
                    self.saved_page = Some((p.page_width, p.page_height));
                }
            } else if let Some((w, h)) = self.saved_page.take() {
                rescale_views(p, (w, h));
                p.page_width = w;
                p.page_height = h;
                self.dirty = true;
            }
        }
    }

    /// Install a freshly opened project, resetting all per-project state
    /// (selection, in-flight gestures, undo history, free-aspect baseline).
    pub fn open_project(&mut self, project: Project, path: Option<PathBuf>) {
        self.project = Some(project);
        self.path = path;
        self.selection = None;
        self.hover = None;
        self.rotate_armed = None;
        self.last_click = None;
        self.drag = None;
        self.pan = None;
        self.dirty = true;
        self.modified = false;
        self.undo.clear();
        self.saved_page = None;
    }

    /// The project as it should be persisted. Free aspect is a *view* mode,
    /// not an edit: while it is on, the model temporarily carries the
    /// canvas-stretched page and viewports, so saving un-stretches a copy
    /// back to the project's own page geometry first.
    pub fn project_for_save(&self) -> Option<std::borrow::Cow<'_, Project>> {
        let p = self.project.as_ref()?;
        match self.saved_page.filter(|_| self.free_aspect) {
            Some((w, h)) if (p.page_width, p.page_height) != (w, h) => {
                let mut copy = p.clone();
                rescale_views(&mut copy, (w, h));
                copy.page_width = w;
                copy.page_height = h;
                Some(std::borrow::Cow::Owned(copy))
            }
            _ => Some(std::borrow::Cow::Borrowed(p)),
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

/// Once a menu-bar menu is open, hovering a sibling bar button switches to
/// it (standard menu-bar UX). egui 0.34's `MenuBar` opens top-level menus
/// on click only — re-evaluate on egui upgrades and delete if `MenuBar`
/// learns this natively.
fn menu_hover_follow(ctx: &egui::Context, bar_buttons: &[egui::Response]) {
    let ids: Vec<egui::Id> = bar_buttons
        .iter()
        .map(egui::Popup::default_response_id)
        .collect();
    if ids.iter().any(|id| egui::Popup::is_id_open(ctx, *id)) {
        for (resp, id) in bar_buttons.iter().zip(&ids) {
            if resp.hovered() && !egui::Popup::is_id_open(ctx, *id) {
                egui::Popup::open_id(ctx, *id);
            }
        }
    }
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

/// Workaround for a winit/egui IME bug seen on recent Wayland compositors: while a
/// text field is focused the compositor streams `Ime(Disabled)` events and delivers
/// every typed character as `Ime(Commit(..))` *without* a preceding `Ime(Enabled)` or
/// `Ime(Preedit)`. egui's `TextEdit` only honors a commit when its (preedit-derived)
/// IME cursor matches the live cursor, and that IME cursor is only updated by
/// `Enabled`/`Preedit` — so it stays at the post-focus position and only the **first**
/// keystroke is accepted; every later one (and any edit of pre-existing text) is
/// silently dropped, though paste and backspace still work. Rewriting each
/// `Ime(Commit(s))` into a plain `Text(s)` event routes it through egui's ungated
/// insertion path, and dropping the stray `Ime` events stops them from confusing the
/// state machine. Our text fields are ASCII/UTF-8 literals, so IME composition isn't
/// needed.
///
/// Linux-only: X11 emits no `Commit` events (characters arrive as `Text`), so this is
/// a no-op there, and macOS/Windows IME (which works) is left untouched.
#[cfg(target_os = "linux")]
fn defuse_broken_ime(ctx: &egui::Context) {
    ctx.input_mut(|i| {
        if !i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))) {
            return;
        }
        for ev in &mut i.events {
            if let egui::Event::Ime(egui::ImeEvent::Commit(s)) = ev {
                let s = std::mem::take(s);
                *ev = egui::Event::Text(s);
            }
        }
        i.events.retain(|e| !matches!(e, egui::Event::Ime(_)));
    });
}

impl eframe::App for App {
    /// Persist [`Settings`] (called by eframe on shutdown and periodically).
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.settings);
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply the theme when the preference or the OS mode changes
        // (App::new already applied the initial Dark).
        let resolved = crate::theme::resolve(ctx, self.theme_pref);
        if resolved != self.theme {
            self.theme = resolved;
            crate::theme::apply(ctx, resolved);
        }
        // Files picked by the async (web) dialog arrive via the inbox.
        #[cfg(target_arch = "wasm32")]
        while let Ok((name, content)) = self.file_rx.try_recv() {
            crate::file::open_loaded(self, name, content);
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
                        // Capture the pre-stretch page lazily, so a project
                        // opened (or launched with -free) while the mode is
                        // already on still restores/saves its own geometry.
                        if self.saved_page.is_none() {
                            self.saved_page = Some((p.page_width, p.page_height));
                        }
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
        // Work around a winit/egui Wayland IME bug that otherwise breaks all text
        // entry (only the first char of a field is accepted). See `defuse_broken_ime`.
        #[cfg(target_os = "linux")]
        defuse_broken_ime(ui.ctx());
        self.menu_bar(ui);
        // Selection changed since the last frame (canvas click, breadcrumb):
        // the tree reveals the newly selected row this frame.
        let reveal = self.selection != self.prev_selection;
        self.prev_selection = self.selection;
        // Note: egui persists panel sizes by id — the two layouts use
        // distinct ids so each remembers its own size (bump the suffix when
        // changing a size policy, or stale stored sizes win).
        if self.settings.inspector_below {
            // Stacked layout: one left panel with the tree above the
            // properties, leaving the full remaining width to the plot.
            egui::Panel::left("tree_stack_v1")
                .resizable(true)
                .default_size(300.0)
                .size_range(220.0..=520.0)
                .show_inside(ui, |ui| {
                    self.toolbar(ui);
                    ui.separator();
                    egui::Panel::bottom("inspector_stack_v1")
                        .resizable(true)
                        .default_size(320.0)
                        .size_range(100.0..=800.0)
                        .show_inside(ui, |ui| self.inspector_panel(ui));
                    egui::CentralPanel::default()
                        .show_inside(ui, |ui| self.tree_panel(ui, reveal));
                });
        } else {
            egui::Panel::left("tree_v2")
                .resizable(true)
                .default_size(220.0)
                .size_range(140.0..=380.0)
                .show_inside(ui, |ui| {
                    self.toolbar(ui);
                    ui.separator();
                    self.tree_panel(ui, reveal);
                });
            egui::Panel::right("inspector_v2")
                .resizable(true)
                .default_size(310.0)
                .size_range(200.0..=400.0)
                .show_inside(ui, |ui| self.inspector_panel(ui));
        }
        self.confirm_close_modal(ui.ctx());
        self.settings_window(ui.ctx());
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

#[cfg(test)]
mod free_aspect_tests {
    use super::*;

    /// Stretching to a free-aspect page and back restores the original
    /// geometry (the un-stretch `project_for_save`/toggle-off relies on).
    #[test]
    fn rescale_views_round_trips() {
        let mut p = Project::default();
        let g = p.graph_mut(0);
        g.view = oxygrace::model::View { xmin: 0.15, xmax: 1.15, ymin: 0.10, ymax: 0.85 };
        let orig = g.view;
        let from = (p.page_width, p.page_height);
        rescale_views(&mut p, (1387, 723));
        p.page_width = 1387;
        p.page_height = 723;
        rescale_views(&mut p, from);
        let v = p.graphs[0].view;
        for (a, b) in [
            (v.xmin, orig.xmin),
            (v.xmax, orig.xmax),
            (v.ymin, orig.ymin),
            (v.ymax, orig.ymax),
        ] {
            assert!((a - b).abs() < 1e-12, "viewport drifted: {a} vs {b}");
        }
    }
}

// Regression test for the Wayland IME workaround (`defuse_broken_ime`). Reproduces the
// broken event stream a recent Wayland/winit combo emits — a flood of `Ime(Disabled)`
// plus one `Ime(Commit)` per keystroke, with no `Enabled`/`Preedit` — which egui's
// `TextEdit` otherwise drops after the first character. Linux-only (the workaround and
// the bug are Linux/Wayland-specific); CI runs on Linux.
#[cfg(all(test, target_os = "linux"))]
mod ime_workaround_tests {
    use super::*;

    fn raw(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 400.0),
            )),
            events,
            ..Default::default()
        }
    }

    fn run(ctx: &egui::Context, text: &mut String, id: egui::Id, events: Vec<egui::Event>) {
        let _ = ctx.run_ui(raw(events), |ui| {
            defuse_broken_ime(ui.ctx());
            ui.add(egui::TextEdit::singleline(text).id(id));
        });
    }

    /// Typing `a`,`b`,`c` arrives as `Ime(Commit)` amid `Ime(Disabled)` noise; with the
    /// workaround every character is inserted (without it, egui keeps only the first).
    #[test]
    fn ime_commit_stream_accumulates_into_empty_field() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("f");
        let mut text = String::new();
        ctx.memory_mut(|m| m.request_focus(id));
        run(&ctx, &mut text, id, vec![egui::Event::Ime(egui::ImeEvent::Disabled)]);
        for ch in ["a", "b", "c"] {
            run(
                &ctx,
                &mut text,
                id,
                vec![
                    egui::Event::Ime(egui::ImeEvent::Disabled),
                    egui::Event::Ime(egui::ImeEvent::Commit(ch.into())),
                    egui::Event::Ime(egui::ImeEvent::Disabled),
                ],
            );
        }
        assert_eq!(text, "abc");
    }

    /// The same stream must also append to *pre-existing* text (the cursor starts > 0,
    /// which is the case egui's commit gate rejects outright).
    #[test]
    fn ime_commit_stream_appends_to_existing_text() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("f");
        let mut text = String::from("all");
        ctx.memory_mut(|m| m.request_focus(id));
        // One frame to place the cursor at the end of the existing text.
        run(&ctx, &mut text, id, vec![]);
        for ch in ["X", "Y"] {
            run(
                &ctx,
                &mut text,
                id,
                vec![egui::Event::Ime(egui::ImeEvent::Commit(ch.into()))],
            );
        }
        assert_eq!(text, "allXY");
    }
}
