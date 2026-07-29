//! The central plot canvas: zoom-to-fit letterboxed blit of the rendered
//! page texture, the screen ↔ device-pixel coordinate mapping, selection,
//! and direct manipulation (drag-move objects, drag-resize the viewport).

use oxygrace::render::{PageTransform, WorldTransform};
use oxygrace::{ElementId, Project};

use crate::app::App;
use crate::edit::Edit;

/// One of the eight resize handles around a selected viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Nw,
    N,
    Ne,
    W,
    E,
    Sw,
    S,
    Se,
}

/// An in-flight pan gesture: the world transform captured at press time
/// (each frame re-pans from it by the total delta, so there is no drift).
#[derive(Clone, Copy)]
pub struct PanState {
    graph: usize,
    start: (f32, f32),
    orig: WorldTransform,
}

/// An in-flight canvas drag, anchored at the press position so each frame
/// recomputes from the original coordinates (no incremental drift).
#[derive(Clone, Copy)]
pub struct DragState {
    id: ElementId,
    kind: DragKind,
    /// Device-pixel position at press.
    start: (f32, f32),
    orig: Orig,
}

#[derive(Clone, Copy)]
enum DragKind {
    Move,
    Resize(Handle),
    /// Move only one endpoint of a line annotation (0 = start, 1 = end).
    Endpoint(u8),
    /// Rotate around the element's anchor (device px), from an original
    /// rotation in degrees.
    Rotate { anchor: (f32, f32), orig_rot: f64 },
}

/// Original model coordinates at press time.
#[derive(Clone, Copy)]
enum Orig {
    /// Single anchor (legend, string, timestamp); `world_graph` is the
    /// owning graph when the position is in world coordinates.
    Point { x: f64, y: f64, world_graph: Option<usize> },
    /// Two anchors (line, box, ellipse).
    TwoPoint { x1: f64, y1: f64, x2: f64, y2: f64, world_graph: Option<usize> },
    /// The graph viewport (move or handle-resize).
    Viewport { xmin: f64, xmax: f64, ymin: f64, ymax: f64, graph: usize },
}

/// Mapping between the on-screen image rect (egui points) and the rendered
/// page's device pixels.
#[derive(Clone, Copy)]
pub struct ViewMap {
    pub img_rect: egui::Rect,
    pub scale: f32,
}

impl ViewMap {
    /// Fit a `page_w` × `page_h` page into `avail`, preserving aspect
    /// (min-scale letterbox, centered).
    pub fn fit(avail: egui::Rect, page_w: u32, page_h: u32) -> Self {
        let scale = (avail.width() / page_w as f32).min(avail.height() / page_h as f32);
        let size = egui::vec2(page_w as f32 * scale, page_h as f32 * scale);
        ViewMap {
            img_rect: egui::Rect::from_center_size(avail.center(), size),
            scale,
        }
    }

    /// Screen position → device pixel.
    pub fn to_device(self, p: egui::Pos2) -> (f32, f32) {
        (
            (p.x - self.img_rect.min.x) / self.scale,
            (p.y - self.img_rect.min.y) / self.scale,
        )
    }

    /// Device pixel → screen position (selection overlay).
    pub fn to_screen(self, x: f32, y: f32) -> egui::Pos2 {
        egui::pos2(
            self.img_rect.min.x + x * self.scale,
            self.img_rect.min.y + y * self.scale,
        )
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let avail = ui.available_rect_before_wrap();
    // Tracked for the free-aspect mode (page follows the canvas size).
    app.canvas_size = (avail.width().max(0.0) as u32, avail.height().max(0.0) as u32);
    let Some(texture) = &app.texture else {
        ui.centered_and_justified(|ui| {
            ui.weak("No plot loaded");
        });
        return;
    };
    let (pw, ph) = app.page_size;
    if pw == 0 || ph == 0 {
        return;
    }
    let vm = ViewMap::fit(avail, pw, ph);
    let resp = ui.allocate_rect(vm.img_rect, egui::Sense::click_and_drag());
    ui.painter().image(
        texture.id(),
        vm.img_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );

    let tol = 6.0 / vm.scale;
    app.hover = None;
    match app.tool {
        crate::app::Tool::Select => select_interactions(app, ui, &resp, vm, tol),
        crate::app::Tool::Pan => handle_pan(app, ui, &resp, vm),
        crate::app::Tool::PickSet => handle_pick_set(app, ui, &resp, vm, tol),
    }

    draw_overlay(app, ui, vm);
}

/// Default-tool interactions: hover status, element drag, click-to-select
/// (with same-spot cycling and rotate arming).
fn select_interactions(app: &mut App, ui: &egui::Ui, resp: &egui::Response, vm: ViewMap, tol: f32) {
    // One hit-test per frame, at the interact position while pressed or
    // clicked (else the hover position — same pointer, same coordinates):
    // the hover preview, the click pick and the drag target all see the
    // same candidate list and cannot disagree.
    let pointer = resp.interact_pointer_pos().or_else(|| resp.hover_pos());
    let (dev, cands) = match pointer {
        Some(pos) => {
            let dev = vm.to_device(pos);
            let cands = app
                .render_info
                .as_ref()
                .map(|info| info.hit_candidates(dev.0, dev.1, tol))
                .unwrap_or_default();
            (dev, cands)
        }
        None => ((0.0, 0.0), Vec::new()),
    };

    // Hover: surface what is under the pointer in the status bar — before
    // any click (~6 screen px of tolerance).
    if resp.hover_pos().is_some() {
        app.hover = cands.first().copied();
        if app.drag.is_none() {
            app.status = match cands.first() {
                Some(&id) => {
                    let d = crate::tree::describe(app.project.as_ref(), id);
                    if cands.len() > 1 {
                        format!("{d}   ({} overlapping — click to cycle)", cands.len())
                    } else {
                        d
                    }
                }
                None => format!("({:.0}, {:.0}) px", dev.0, dev.1),
            };
        }
    }
    handle_drag(app, ui, resp, vm, &cands);

    if resp.clicked()
        && !on_selection_handle(app, resp, vm)
        && resp.interact_pointer_pos().is_some()
    {
        let (dx, dy) = dev;
        // Clicking the same spot again cycles through overlapping
        // elements (frame under plot area, axis under frame, …).
        let same_spot = app
            .last_click
            .is_some_and(|(lx, ly)| (lx - dx).hypot(ly - dy) * vm.scale < 6.0);
        let pick = match app
            .selection
            .filter(|_| same_spot)
            .and_then(|s| cands.iter().position(|&c| c == s))
        {
            // Second click on a rotatable selection toggles rotate mode
            // instead of cycling.
            Some(_) if app.selection.is_some_and(rotatable) => {
                let sel = app.selection.unwrap();
                app.rotate_armed = if app.rotate_armed == Some(sel) { None } else { Some(sel) };
                Some(sel)
            }
            Some(i) if !cands.is_empty() => Some(cands[(i + 1) % cands.len()]),
            _ => cands.first().copied(),
        };
        if pick != app.selection {
            app.rotate_armed = None;
        }
        app.selection = pick;
        app.last_click = Some((dx, dy));
        app.status = match pick {
            Some(id) => {
                let d = crate::tree::describe(app.project.as_ref(), id);
                if cands.len() > 1 {
                    format!("{d}   ({} overlapping — click again to cycle)", cands.len())
                } else {
                    d
                }
            }
            None => "No selection".into(),
        };
    }
}

/// Pan tool: drag to shift the world window of the graph under the cursor.
fn handle_pan(app: &mut App, ui: &egui::Ui, resp: &egui::Response, vm: ViewMap) {
    if app.pan.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if resp.hover_pos().is_some() && app.pan.is_none() {
        app.status = "Pan: drag to move the view".into();
    }

    if resp.drag_started() {
        app.pan = None;
        if let (Some(pos), Some(project)) = (resp.interact_pointer_pos(), app.project.as_ref()) {
            let (dx, dy) = vm.to_device(pos);
            if let Some(graph) = graph_at(app, dx, dy) {
                app.pan = Some(PanState {
                    graph,
                    start: (dx, dy),
                    orig: WorldTransform::new(&project.graphs[graph]),
                });
            }
        }
    }
    if let Some(pan) = app.pan {
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let (dvx, dvy) = view_delta(app, pan.start, vm.to_device(pos));
                let (x0, x1, y0, y1) = pan.orig.pan_world(dvx, dvy);
                let g = pan.graph;
                app.apply_edit(Edit::new("pan", (x0, x1, y0, y1), true, move |p, (a, b, c, d)| {
                    if let Some(gr) = p.graphs.get_mut(g) {
                        gr.world.xmin = a;
                        gr.world.xmax = b;
                        gr.world.ymin = c;
                        gr.world.ymax = d;
                    }
                }));
                app.status = format!("World {x0:.4} … {x1:.4},  {y0:.4} … {y1:.4}");
            }
        }
        if resp.drag_stopped() {
            app.pan = None;
            app.end_gesture();
        }
    }
}

/// Autoscale-to-set tool: click a set to fit its graph to that set.
fn handle_pick_set(app: &mut App, ui: &egui::Ui, resp: &egui::Response, vm: ViewMap, tol: f32) {
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        app.status = "Click a set to autoscale its graph to it".into();
    }
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let (dx, dy) = vm.to_device(pos);
            let pick = app
                .render_info
                .as_ref()
                .map(|info| info.hit_candidates(dx, dy, tol))
                .unwrap_or_default()
                .into_iter()
                .find_map(|id| match id {
                    ElementId::Set { graph, set } => Some((graph, set)),
                    _ => None,
                });
            match pick {
                Some((g, s)) => {
                    app.selection = Some(ElementId::Set { graph: g, set: s });
                    app.autoscale_to_set(g, s);
                }
                None => app.status = "No set there — click directly on a set".into(),
            }
        }
    }
}

/// The page's view↔device mapping for the last render (the core transform
/// the renderer itself used — plot_view never re-derives this math).
fn page_transform(app: &App) -> PageTransform {
    PageTransform::new(app.page_size.0, app.page_size.1)
}

/// View-space displacement between two device points (drag/pan deltas).
fn view_delta(app: &App, from: (f32, f32), to: (f32, f32)) -> (f64, f64) {
    let pt = page_transform(app);
    let (x0, y0) = pt.device_to_view(from.0, from.1);
    let (x1, y1) = pt.device_to_view(to.0, to.1);
    (x1 - x0, y1 - y0)
}

/// The graph whose viewport contains device point `(dx, dy)` — topmost
/// (last-drawn) visible graph wins.
fn graph_at(app: &App, dx: f32, dy: f32) -> Option<usize> {
    let project = app.project.as_ref()?;
    let (vx, vy) = page_transform(app).device_to_view(dx, dy);
    for (i, g) in project.graphs.iter().enumerate().rev() {
        if g.hidden {
            continue;
        }
        let v = g.view;
        if vx >= v.xmin && vx <= v.xmax && vy >= v.ymin && vy <= v.ymax {
            return Some(i);
        }
    }
    None
}

/// Elements that can be dragged to a new position.
fn draggable(id: ElementId) -> bool {
    use ElementId::*;
    matches!(
        id,
        Legend(_) | StringObj(_) | LineObj(_) | BoxObj(_) | EllipseObj(_) | Timestamp
    )
}

/// Elements with a rotation property (second click arms rotate mode).
fn rotatable(id: ElementId) -> bool {
    matches!(id, ElementId::StringObj(_) | ElementId::Timestamp)
}

/// Anchor position and rotation of a rotatable element.
fn rotation_of(project: &Project, id: ElementId) -> Option<(f64, f64, Option<usize>, f64)> {
    match id {
        ElementId::StringObj(i) => {
            let s = project.strings.get(i)?;
            Some((s.x, s.y, (!s.loctype_view).then_some(s.gno), s.rot))
        }
        ElementId::Timestamp => {
            let t = &project.timestamp;
            Some((t.x, t.y, None, t.rot))
        }
        _ => None,
    }
}

/// Convert possibly world-anchored model coordinates to view coordinates.
fn to_view_coords(project: &Project, world_graph: Option<usize>, x: f64, y: f64) -> Option<(f64, f64)> {
    match world_graph {
        None => Some((x, y)),
        Some(g) => Some(WorldTransform::new(project.graphs.get(g)?).world_to_view(x, y)),
    }
}

/// View coordinates → screen position (through device pixels).
fn view_to_screen(app: &App, vm: ViewMap, vx: f64, vy: f64) -> egui::Pos2 {
    let (dx, dy) = page_transform(app).view_to_device(vx, vy);
    vm.to_screen(dx, dy)
}

/// Screen positions of a line annotation's two endpoints.
fn line_endpoints_screen(app: &App, vm: ViewMap, i: usize) -> Option<(egui::Pos2, egui::Pos2)> {
    let project = app.project.as_ref()?;
    let l = project.lines.get(i)?;
    let wg = (!l.loctype_view).then_some(l.gno);
    let (vx1, vy1) = to_view_coords(project, wg, l.x1, l.y1)?;
    let (vx2, vy2) = to_view_coords(project, wg, l.x2, l.y2)?;
    Some((view_to_screen(app, vm, vx1, vy1), view_to_screen(app, vm, vx2, vy2)))
}

/// Original model coordinates for a drag of `id`.
fn orig_of(project: &Project, id: ElementId) -> Option<Orig> {
    use ElementId::*;
    Some(match id {
        Legend(g) => {
            let l = &project.graphs.get(g)?.legend;
            Orig::Point { x: l.x, y: l.y, world_graph: (!l.loctype_view).then_some(g) }
        }
        StringObj(i) => {
            let s = project.strings.get(i)?;
            Orig::Point { x: s.x, y: s.y, world_graph: (!s.loctype_view).then_some(s.gno) }
        }
        Timestamp => {
            let t = &project.timestamp;
            Orig::Point { x: t.x, y: t.y, world_graph: None }
        }
        LineObj(i) => {
            let l = project.lines.get(i)?;
            Orig::TwoPoint {
                x1: l.x1,
                y1: l.y1,
                x2: l.x2,
                y2: l.y2,
                world_graph: (!l.loctype_view).then_some(l.gno),
            }
        }
        BoxObj(i) | EllipseObj(i) => {
            let b = if matches!(id, BoxObj(_)) {
                project.boxes.get(i)?
            } else {
                project.ellipses.get(i)?
            };
            Orig::TwoPoint {
                x1: b.x1,
                y1: b.y1,
                x2: b.x2,
                y2: b.y2,
                world_graph: (!b.loctype_view).then_some(b.gno),
            }
        }
        Graph(g) | Frame(g) => {
            let v = project.graphs.get(g)?.view;
            Orig::Viewport { xmin: v.xmin, xmax: v.xmax, ymin: v.ymin, ymax: v.ymax, graph: g }
        }
        _ => return None,
    })
}

/// The eight handle anchor points of a selection rect, with their handles.
fn handle_points(r: egui::Rect) -> [(Handle, egui::Pos2); 8] {
    [
        (Handle::Nw, r.left_top()),
        (Handle::N, r.center_top()),
        (Handle::Ne, r.right_top()),
        (Handle::W, r.left_center()),
        (Handle::E, r.right_center()),
        (Handle::Sw, r.left_bottom()),
        (Handle::S, r.center_bottom()),
        (Handle::Se, r.right_bottom()),
    ]
}

/// Which handle of the selected Graph/Frame (if any) is at `pos`.
fn handle_at(app: &App, pos: egui::Pos2, vm: ViewMap) -> Option<Handle> {
    let sel = app.selection.filter(|s| matches!(s, ElementId::Graph(_) | ElementId::Frame(_)))?;
    let info = app.render_info.as_ref()?;
    let r = bounds_on_screen(info, sel, vm)?.expand(3.0);
    handle_points(r)
        .into_iter()
        .find(|(_, p)| egui::Rect::from_center_size(*p, egui::vec2(14.0, 14.0)).contains(pos))
        .map(|(h, _)| h)
}

fn on_selection_handle(app: &App, resp: &egui::Response, vm: ViewMap) -> bool {
    resp.interact_pointer_pos()
        .is_some_and(|pos| handle_at(app, pos, vm).is_some())
}

fn cursor_for(h: Handle) -> egui::CursorIcon {
    use egui::CursorIcon::*;
    match h {
        Handle::N | Handle::S => ResizeVertical,
        Handle::E | Handle::W => ResizeHorizontal,
        Handle::Ne | Handle::Sw => ResizeNeSw,
        Handle::Nw | Handle::Se => ResizeNwSe,
    }
}

/// Drag interactions: start on press (handle-resize first, then a move of a
/// draggable element under the pointer), update with coalescing edits while
/// dragging, end the undo gesture on release. `cands` is the frame's shared
/// hit-candidate list (computed once in [`select_interactions`]).
fn handle_drag(app: &mut App, ui: &egui::Ui, resp: &egui::Response, vm: ViewMap, cands: &[ElementId]) {
    // Cursor feedback.
    if app.drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if let Some(pos) = resp.hover_pos() {
        if let Some(h) = handle_at(app, pos, vm) {
            ui.ctx().set_cursor_icon(cursor_for(h));
        } else if app.hover.is_some_and(draggable) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    if resp.drag_started() {
        app.drag = None;
        let Some(pos) = resp.interact_pointer_pos() else { return };
        let Some(project) = &app.project else { return };
        let dev = vm.to_device(pos);
        // Rotate mode: dragging near the armed element rotates it.
        if let Some(sel) = app.rotate_armed.filter(|s| app.selection == Some(*s)) {
            if let Some((x, y, wg, rot)) = rotation_of(project, sel) {
                let near = app
                    .render_info
                    .as_ref()
                    .and_then(|info| bounds_on_screen(info, sel, vm))
                    .is_some_and(|r| r.expand(24.0).contains(pos));
                if near {
                    if let Some((vx, vy)) = to_view_coords(project, wg, x, y) {
                        let a = view_to_screen(app, vm, vx, vy);
                        let anchor = vm.to_device(a);
                        app.drag = Some(DragState {
                            id: sel,
                            kind: DragKind::Rotate { anchor, orig_rot: rot },
                            start: dev,
                            orig: Orig::Point { x, y, world_graph: wg },
                        });
                        return;
                    }
                }
            }
        }
        // Endpoint handles of a selected line annotation.
        if let Some(ElementId::LineObj(i)) = app.selection {
            if let Some((p1, p2)) = line_endpoints_screen(app, vm, i) {
                let near = |p: egui::Pos2| (p - pos).length() < 12.0;
                let which = if near(p1) {
                    Some(0u8)
                } else if near(p2) {
                    Some(1u8)
                } else {
                    None
                };
                if let Some(which) = which {
                    if let Some(orig) = orig_of(project, ElementId::LineObj(i)) {
                        app.drag = Some(DragState {
                            id: ElementId::LineObj(i),
                            kind: DragKind::Endpoint(which),
                            start: dev,
                            orig,
                        });
                        return;
                    }
                }
            }
        }
        // Resize handles of a selected viewport take priority.
        if let Some(h) = handle_at(app, pos, vm) {
            if let Some(orig) = app.selection.and_then(|s| orig_of(project, s)) {
                app.drag = Some(DragState {
                    id: app.selection.unwrap(),
                    kind: DragKind::Resize(h),
                    start: dev,
                    orig,
                });
                return;
            }
        }
        // Otherwise move a draggable element under the pointer (preferring
        // the current selection so drags don't jump to occluders).
        let target = app
            .selection
            .filter(|s| draggable(*s) && cands.contains(s))
            .or_else(|| cands.iter().copied().find(|c| draggable(*c)));
        if let Some(id) = target {
            if let Some(orig) = orig_of(project, id) {
                app.selection = Some(id);
                app.drag = Some(DragState { id, kind: DragKind::Move, start: dev, orig });
            }
        }
    }

    if let Some(drag) = app.drag {
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let dev = vm.to_device(pos);
                let (dvx, dvy) = view_delta(app, drag.start, dev);
                apply_drag(app, drag, dvx, dvy, dev);
            }
        }
        if resp.drag_stopped() {
            app.drag = None;
            app.end_gesture();
        }
    }
}

/// Shift a point by a view delta, through world coordinates when the
/// position is world-anchored.
fn shifted(
    project: &Project,
    world_graph: Option<usize>,
    x: f64,
    y: f64,
    dvx: f64,
    dvy: f64,
) -> Option<(f64, f64)> {
    match world_graph {
        None => Some((x + dvx, y + dvy)),
        Some(g) => {
            let wt = WorldTransform::new(project.graphs.get(g)?);
            let (vx, vy) = wt.world_to_view(x, y);
            wt.view_to_world(vx + dvx, vy + dvy)
        }
    }
}

fn apply_drag(app: &mut App, drag: DragState, dvx: f64, dvy: f64, dev: (f32, f32)) {
    let Some(project) = &app.project else { return };
    let id = drag.id;
    // Rotation works on device angles around the element's anchor.
    if let DragKind::Rotate { anchor, orig_rot } = drag.kind {
        let a0 = (drag.start.1 - anchor.1).atan2(drag.start.0 - anchor.0);
        let a1 = (dev.1 - anchor.1).atan2(dev.0 - anchor.0);
        // Device Y points down; Grace rotations are CCW degrees.
        let rot = orig_rot + (a0 - a1).to_degrees() as f64;
        let rot = if rot > 180.0 { rot - 360.0 } else if rot < -180.0 { rot + 360.0 } else { rot };
        app.apply_edit(Edit::new("rotate text", rot, true, move |p, r| set_rot(p, id, r)));
        app.status = format!("Rotation {rot:.1}°");
        return;
    }
    match drag.orig {
        Orig::Point { x, y, world_graph } => {
            let Some((nx, ny)) = shifted(project, world_graph, x, y, dvx, dvy) else { return };
            app.apply_edit(Edit::new(move_label(id), (nx, ny), true, move |p, (x, y)| {
                set_point(p, id, x, y);
            }));
            app.status = format!("{} → ({nx:.4}, {ny:.4})", crate::tree::describe(None, id));
        }
        Orig::TwoPoint { x1, y1, x2, y2, world_graph } => {
            // Endpoint drags shift one end; body drags shift both.
            let (move1, move2) = match drag.kind {
                DragKind::Endpoint(0) => (true, false),
                DragKind::Endpoint(_) => (false, true),
                _ => (true, true),
            };
            let shift1 = if move1 { (dvx, dvy) } else { (0.0, 0.0) };
            let shift2 = if move2 { (dvx, dvy) } else { (0.0, 0.0) };
            let Some((nx1, ny1)) = shifted(project, world_graph, x1, y1, shift1.0, shift1.1) else {
                return;
            };
            let Some((nx2, ny2)) = shifted(project, world_graph, x2, y2, shift2.0, shift2.1) else {
                return;
            };
            app.apply_edit(Edit::new(
                move_label(id),
                (nx1, ny1, nx2, ny2),
                true,
                move |p, (x1, y1, x2, y2)| set_two_points(p, id, x1, y1, x2, y2),
            ));
        }
        Orig::Viewport { xmin, xmax, ymin, ymax, graph } => {
            let (mut x0, mut x1, mut y0, mut y1) = (xmin, xmax, ymin, ymax);
            match drag.kind {
                DragKind::Endpoint(_) | DragKind::Rotate { .. } => return,
                DragKind::Move => {
                    x0 += dvx;
                    x1 += dvx;
                    y0 += dvy;
                    y1 += dvy;
                }
                DragKind::Resize(h) => {
                    // Screen-top handles move view ymax (view Y is up).
                    use Handle::*;
                    if matches!(h, Nw | W | Sw) {
                        x0 += dvx;
                    }
                    if matches!(h, Ne | E | Se) {
                        x1 += dvx;
                    }
                    if matches!(h, Nw | N | Ne) {
                        y1 += dvy;
                    }
                    if matches!(h, Sw | S | Se) {
                        y0 += dvy;
                    }
                }
            }
            // Keep a sliver of viewport so the transform stays sane.
            if x1 - x0 < 0.02 || y1 - y0 < 0.02 {
                return;
            }
            app.apply_edit(Edit::new(
                "resize viewport",
                (x0, x1, y0, y1),
                true,
                move |p, (x0, x1, y0, y1)| {
                    if let Some(gr) = p.graphs.get_mut(graph) {
                        gr.view.xmin = x0;
                        gr.view.xmax = x1;
                        gr.view.ymin = y0;
                        gr.view.ymax = y1;
                    }
                },
            ));
            app.status = format!("Viewport {x0:.3}, {y0:.3} … {x1:.3}, {y1:.3}");
        }
    }
}

fn move_label(id: ElementId) -> &'static str {
    use ElementId::*;
    match id {
        Legend(_) => "move legend",
        StringObj(_) => "move text",
        LineObj(_) => "move line",
        BoxObj(_) => "move box",
        EllipseObj(_) => "move ellipse",
        Timestamp => "move timestamp",
        Graph(_) | Frame(_) => "move viewport",
        _ => "move element",
    }
}

fn set_point(p: &mut Project, id: ElementId, x: f64, y: f64) {
    use ElementId::*;
    match id {
        Legend(g) => {
            if let Some(gr) = p.graphs.get_mut(g) {
                gr.legend.x = x;
                gr.legend.y = y;
            }
        }
        StringObj(i) => {
            if let Some(s) = p.strings.get_mut(i) {
                s.x = x;
                s.y = y;
            }
        }
        Timestamp => {
            p.timestamp.x = x;
            p.timestamp.y = y;
        }
        _ => {}
    }
}

fn set_rot(p: &mut Project, id: ElementId, rot: f64) {
    match id {
        ElementId::StringObj(i) => {
            if let Some(s) = p.strings.get_mut(i) {
                s.rot = rot;
            }
        }
        ElementId::Timestamp => p.timestamp.rot = rot,
        _ => {}
    }
}

fn set_two_points(p: &mut Project, id: ElementId, x1: f64, y1: f64, x2: f64, y2: f64) {
    use ElementId::*;
    let obj = match id {
        LineObj(i) => {
            if let Some(l) = p.lines.get_mut(i) {
                (l.x1, l.y1, l.x2, l.y2) = (x1, y1, x2, y2);
            }
            return;
        }
        BoxObj(i) => p.boxes.get_mut(i),
        EllipseObj(i) => p.ellipses.get_mut(i),
        _ => None,
    };
    if let Some(b) = obj {
        (b.x1, b.y1, b.x2, b.y2) = (x1, y1, x2, y2);
    }
}

/// Elements highlighted by tracing their actual recorded geometry (lines,
/// symbols, label boxes) rather than one big bounding box. Sets are
/// handled separately again: one contour around *all* their ink.
fn shape_highlighted(id: oxygrace::ElementId) -> bool {
    use oxygrace::ElementId::*;
    matches!(id, AxisBar { .. } | TickLabels { .. } | LineObj(_))
}

/// Gap between an element's ink and its highlight outline: the outline is
/// drawn *around* the shapes, never on them, so the element's own color
/// and line width stay visible while they are edited.
const HIGHLIGHT_GAP: f32 = 3.5;

/// Selection and hover highlights, painted on top of the texture (never
/// baked into the pixmap, so they cost no re-render).
///
/// Every stroke sits on a white halo so it reads crisply over any plot
/// content. Data-like elements (sets, axes, tick labels, line objects) are
/// highlighted along their actual shape; box-like elements (legend, titles,
/// annotations) get a bounds box with handles.
fn draw_overlay(app: &App, ui: &egui::Ui, vm: ViewMap) {
    let Some(info) = &app.render_info else { return };
    // Clip slightly beyond the page image so the halo isn't cut off.
    let painter = ui.painter_at(vm.img_rect.expand(8.0));
    // Overlay ink from the live theme's sheet (`[extras]`): drawn over the
    // rendered page, so it answers to the paper, not to the panels.
    let ex = crate::theme::extras(ui.ctx());
    let (accent, halo) = (ex.accent, ex.halo);

    // Hover highlight (skip when same as selection).
    if let Some(id) = app.hover.filter(|h| app.selection != Some(*h)) {
        highlight(&painter, info, id, vm, 0.55, ex);
    }
    if let Some(id) = app.selection {
        highlight(&painter, info, id, vm, 1.0, ex);
    }

    // Endpoint handles of a selected line annotation (drag affordances).
    if let Some(ElementId::LineObj(i)) = app.selection {
        if let Some((p1, p2)) = line_endpoints_screen(app, vm, i) {
            for p in [p1, p2] {
                let hr = egui::Rect::from_center_size(p, egui::vec2(10.0, 10.0));
                painter.rect_filled(hr, 1.0, halo);
                painter.rect_stroke(hr, 1.0, egui::Stroke::new(2.0_f32, accent), egui::StrokeKind::Inside);
            }
        }
    }
    // Rotate markers: circles at the corners while rotate mode is armed.
    if let Some(sel) = app.rotate_armed.filter(|s| app.selection == Some(*s)) {
        if let Some(r) = bounds_on_screen(info, sel, vm) {
            let r = r.expand(8.0);
            for p in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
                painter.circle_filled(p, 6.0, halo);
                painter.circle_stroke(p, 6.0, egui::Stroke::new(2.0_f32, accent));
            }
        }
    }
}

/// Draw one element's highlight at the given intensity (hover is fainter).
fn highlight(
    painter: &egui::Painter,
    info: &oxygrace::RenderInfo,
    id: oxygrace::ElementId,
    vm: ViewMap,
    intensity: f32,
    ex: crate::theme::Extras,
) {
    let halo = ex.halo.gamma_multiply(0.8 * intensity);
    let accent = ex.accent.gamma_multiply(intensity);

    // Sets: closed contours around *all* the ink at once (distance-field
    // isoline) — the outline follows the data's shape at a fixed distance
    // without tracing, and so masking, any individual point or segment.
    if let oxygrace::ElementId::Set { .. } = id {
        for lp in set_outline_loops(info, id, vm) {
            painter.add(egui::Shape::closed_line(lp.clone(), egui::Stroke::new(3.5_f32, halo)));
            painter.add(egui::Shape::closed_line(lp, egui::Stroke::new(1.5_f32, accent)));
        }
        return;
    }

    if shape_highlighted(id) {
        for (clip, shape) in info.shapes_of(id) {
            // Respect the clip the primitive was drawn under, so highlights
            // of clipped data don't stretch outside the graph viewport.
            let painter = &match clip {
                Some(c) => {
                    let r = bounds_to_screen(c, vm);
                    painter.with_clip_rect(r.intersect(painter.clip_rect()))
                }
                None => painter.clone(),
            };
            // A closed outline drawn a fixed gap outside the ink.
            let outline = |pts: Vec<egui::Pos2>| {
                painter.add(egui::Shape::closed_line(
                    pts.clone(),
                    egui::Stroke::new(3.5_f32, halo),
                ));
                painter.add(egui::Shape::closed_line(pts, egui::Stroke::new(1.5_f32, accent)));
            };
            match shape {
                oxygrace::OverlayShape::Lines { pts, half_width } => {
                    let line = screen_polyline(pts, vm);
                    if line.len() < 2 {
                        continue;
                    }
                    let w = (2.0 * half_width * vm.scale).max(1.5);
                    outline(outline_around_polyline(&line, w / 2.0 + HIGHLIGHT_GAP));
                }
                oxygrace::OverlayShape::Polygon(pts) => {
                    let line = screen_polyline(pts, vm);
                    if line.len() < 3 {
                        continue;
                    }
                    outline(outline_around_polygon(&line, HIGHLIGHT_GAP));
                }
                oxygrace::OverlayShape::Rect(b) => {
                    let r = bounds_to_screen(b, vm).expand(HIGHLIGHT_GAP);
                    painter.rect_stroke(r, 1.0, egui::Stroke::new(3.5_f32, halo), egui::StrokeKind::Outside);
                    painter.rect_stroke(r, 1.0, egui::Stroke::new(1.5_f32, accent), egui::StrokeKind::Outside);
                }
            }
        }
        return;
    }

    // Box-like elements: bounds box (+ handles when selected, not hovered).
    let Some(r) = bounds_on_screen(info, id, vm) else { return };
    let r = r.expand(3.0);
    painter.rect_stroke(r, 0.0, egui::Stroke::new(5.0_f32, halo), egui::StrokeKind::Outside);
    painter.rect_stroke(r, 0.0, egui::Stroke::new(2.5_f32, accent), egui::StrokeKind::Outside);
    if intensity >= 1.0 {
        for p in [
            r.left_top(),
            r.center_top(),
            r.right_top(),
            r.left_center(),
            r.right_center(),
            r.left_bottom(),
            r.center_bottom(),
            r.right_bottom(),
        ] {
            let hr = egui::Rect::from_center_size(p, egui::vec2(10.0, 10.0));
            painter.rect_filled(hr, 1.0, ex.halo);
            painter.rect_stroke(hr, 1.0, egui::Stroke::new(2.0_f32, ex.accent), egui::StrokeKind::Inside);
        }
    }
}

/// Ink → set-outline distance (px): the isoline level of the contour
/// drawn around a selected set's ink.
const SET_GAP: f32 = 6.0;
/// Resolution of the set-outline distance field (px per cell; grows when
/// a set spans more than ~500 cells).
const FIELD_CELL: f32 = 3.0;

/// A coarse screen-space "distance to the ink" field over one set.
struct DistGrid {
    x0: f32,
    y0: f32,
    cell: f32,
    w: usize,
    h: usize,
    v: Vec<f32>,
}

impl DistGrid {
    const FAR: f32 = 1e9;

    fn new(bbox: egui::Rect, cell: f32) -> Self {
        let w = (bbox.width() / cell).ceil() as usize + 2;
        let h = (bbox.height() / cell).ceil() as usize + 2;
        DistGrid { x0: bbox.min.x, y0: bbox.min.y, cell, w, h, v: vec![Self::FAR; w * h] }
    }

    fn center(&self, i: usize, j: usize) -> egui::Pos2 {
        egui::pos2(self.x0 + i as f32 * self.cell, self.y0 + j as f32 * self.cell)
    }

    /// Lower the field to `dist(cell, p) - r` for cells within `reach`.
    fn stamp(&mut self, p: egui::Pos2, r: f32, reach: f32) {
        let i0 = (((p.x - reach - self.x0) / self.cell).floor().max(0.0)) as usize;
        let j0 = (((p.y - reach - self.y0) / self.cell).floor().max(0.0)) as usize;
        let i1 = ((((p.x + reach - self.x0) / self.cell).ceil()) as usize).min(self.w - 1);
        let j1 = ((((p.y + reach - self.y0) / self.cell).ceil()) as usize).min(self.h - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let d = (self.center(i, j) - p).length() - r;
                let v = &mut self.v[j * self.w + i];
                if d < *v {
                    *v = d;
                }
            }
        }
    }

    /// Scanline-fill a polygon's interior to distance 0, so filled areas
    /// (set fills, bars) get only an outer contour, not an inner ring.
    fn fill_polygon(&mut self, poly: &[egui::Pos2]) {
        for j in 0..self.h {
            let y = self.y0 + j as f32 * self.cell;
            let mut xs: Vec<f32> = Vec::new();
            for k in 0..poly.len() {
                let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                if (a.y > y) != (b.y > y) {
                    xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
            xs.sort_by(f32::total_cmp);
            for pair in xs.chunks_exact(2) {
                let i0 = (((pair[0] - self.x0) / self.cell).ceil().max(0.0)) as usize;
                let i1 = ((((pair[1] - self.x0) / self.cell).floor()) as usize).min(self.w - 1);
                for i in i0..=i1 {
                    self.v[j * self.w + i] = self.v[j * self.w + i].min(0.0);
                }
            }
        }
    }
}

/// Closed contour loops around *all* of a set's ink at [`SET_GAP`]: stamp
/// a distance field from every recorded shape, then trace its `SET_GAP`
/// isoline with marching squares. Disconnected clusters give separate
/// loops and ring-shaped data keeps its hole — and the outline never sits
/// on the ink itself.
fn set_outline_loops(
    info: &oxygrace::RenderInfo,
    id: oxygrace::ElementId,
    vm: ViewMap,
) -> Vec<Vec<egui::Pos2>> {
    // 1. Collect stamp samples (position + ink half-width) and filled
    //    polygons in screen space; clipped-away geometry contributes none.
    let mut samples: Vec<(egui::Pos2, f32)> = Vec::new();
    let mut fills: Vec<Vec<egui::Pos2>> = Vec::new();
    for (clip, shape) in info.shapes_of(id) {
        let clip_r = clip.map(|c| bounds_to_screen(c, vm).expand(2.0));
        let keep = |p: egui::Pos2| clip_r.is_none_or(|r| r.contains(p));
        match shape {
            oxygrace::OverlayShape::Lines { pts, half_width } => {
                let r = (half_width * vm.scale).max(0.75);
                sample_path(&screen_polyline(pts, vm), false, |p| {
                    if keep(p) {
                        samples.push((p, r));
                    }
                });
            }
            oxygrace::OverlayShape::Polygon(pts) => {
                let poly = screen_polyline(pts, vm);
                if poly.len() >= 3 {
                    sample_path(&poly, true, |p| {
                        if keep(p) {
                            samples.push((p, 0.75));
                        }
                    });
                    fills.push(poly);
                }
            }
            oxygrace::OverlayShape::Rect(b) => {
                // Sample the box area at cell steps (symbol boxes are a few
                // cells; avalue label boxes stay hole-free).
                let r = bounds_to_screen(b, vm);
                let (nx, ny) = (
                    (r.width() / FIELD_CELL).ceil().max(1.0) as usize,
                    (r.height() / FIELD_CELL).ceil().max(1.0) as usize,
                );
                for j in 0..=ny {
                    for i in 0..=nx {
                        let p = egui::pos2(
                            r.min.x + r.width() * i as f32 / nx as f32,
                            r.min.y + r.height() * j as f32 / ny as f32,
                        );
                        if keep(p) {
                            samples.push((p, 0.75));
                        }
                    }
                }
            }
        }
    }
    if samples.is_empty() {
        return Vec::new();
    }

    // 2. Field extent and resolution (bounded grid: huge sets coarsen).
    let mut bbox = egui::Rect::NOTHING;
    for (p, _) in &samples {
        bbox.extend_with(*p);
    }
    let margin = SET_GAP + 4.0 * FIELD_CELL;
    let bbox = bbox.expand(margin);
    let cell = FIELD_CELL.max(bbox.width() / 500.0).max(bbox.height() / 500.0);
    let mut grid = DistGrid::new(bbox, cell);

    // 3. Stamp (deduplicated per cell — dense clouds cost grid-area, not
    //    point-count).
    let mut seen = std::collections::HashSet::new();
    for &(p, r) in &samples {
        let key = (
            ((p.x - grid.x0) / cell) as i32,
            ((p.y - grid.y0) / cell) as i32,
            (r * 2.0) as i32,
        );
        if seen.insert(key) {
            grid.stamp(p, r, SET_GAP + r + 3.0 * cell);
        }
    }
    for poly in &fills {
        grid.fill_polygon(poly);
    }

    // 4. Trace the isoline.
    marching_squares(&grid, SET_GAP)
}

/// Call `f` at steps of [`FIELD_CELL`] along a polyline (`closed` adds the
/// last→first segment).
fn sample_path(pts: &[egui::Pos2], closed: bool, mut f: impl FnMut(egui::Pos2)) {
    let n = pts.len();
    if n == 0 {
        return;
    }
    let last = if closed { n } else { n - 1 };
    for k in 0..last {
        let (a, b) = (pts[k], pts[(k + 1) % n]);
        let steps = ((b - a).length() / FIELD_CELL).ceil().max(1.0) as usize;
        for s in 0..steps {
            f(a + (b - a) * (s as f32 / steps as f32));
        }
    }
    f(pts[n - 1]);
}

/// Trace the `iso` contour of a distance field as closed loops (marching
/// squares with linear interpolation, segments chained by shared
/// endpoints).
fn marching_squares(grid: &DistGrid, iso: f32) -> Vec<Vec<egui::Pos2>> {
    let at = |i: usize, j: usize| grid.v[j * grid.w + i];
    let mut segs: Vec<(egui::Pos2, egui::Pos2)> = Vec::new();
    for j in 0..grid.h - 1 {
        for i in 0..grid.w - 1 {
            // Corner values: 0 = (i,j), 1 = (i+1,j), 2 = (i+1,j+1), 3 = (i,j+1).
            let v = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            let mut case = 0usize;
            for (k, val) in v.iter().enumerate() {
                if *val <= iso {
                    case |= 1 << k;
                }
            }
            if case == 0 || case == 15 {
                continue;
            }
            let corner = [
                grid.center(i, j),
                grid.center(i + 1, j),
                grid.center(i + 1, j + 1),
                grid.center(i, j + 1),
            ];
            // Interpolated crossing on the edge between corners a and b.
            let cross = |a: usize, b: usize| {
                let t = ((iso - v[a]) / (v[b] - v[a])).clamp(0.0, 1.0);
                corner[a] + (corner[b] - corner[a]) * t
            };
            let (bottom, right, top, left) =
                (|| cross(0, 1), || cross(1, 2), || cross(3, 2), || cross(0, 3));
            match case {
                1 | 14 => segs.push((bottom(), left())),
                2 | 13 => segs.push((bottom(), right())),
                4 | 11 => segs.push((right(), top())),
                8 | 7 => segs.push((top(), left())),
                3 | 12 => segs.push((left(), right())),
                6 | 9 => segs.push((bottom(), top())),
                5 => {
                    segs.push((bottom(), left()));
                    segs.push((right(), top()));
                }
                10 => {
                    segs.push((bottom(), right()));
                    segs.push((top(), left()));
                }
                _ => unreachable!(),
            }
        }
    }
    chain_loops(segs)
}

/// Chain marching-squares segments into closed loops. Crossing points on a
/// shared cell edge are computed from the same values in the same order by
/// both cells, so endpoints match bit-exactly.
fn chain_loops(segs: Vec<(egui::Pos2, egui::Pos2)>) -> Vec<Vec<egui::Pos2>> {
    let key = |p: egui::Pos2| (p.x.to_bits(), p.y.to_bits());
    let mut adj: std::collections::HashMap<(u32, u32), Vec<usize>> = std::collections::HashMap::new();
    for (idx, (a, b)) in segs.iter().enumerate() {
        adj.entry(key(*a)).or_default().push(idx);
        adj.entry(key(*b)).or_default().push(idx);
    }
    let mut used = vec![false; segs.len()];
    let mut loops = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (a, b) = segs[start];
        let mut path = vec![a, b];
        let mut cur = b;
        loop {
            let next = adj
                .get(&key(cur))
                .and_then(|c| c.iter().copied().find(|&s| !used[s]));
            let Some(next) = next else { break };
            used[next] = true;
            let (na, nb) = segs[next];
            cur = if key(na) == key(cur) { nb } else { na };
            if key(cur) == key(path[0]) {
                break; // loop closed
            }
            path.push(cur);
        }
        if path.len() >= 3 {
            loops.push(path);
        }
    }
    loops
}

/// A closed loop around an open polyline at distance `d`: one side's
/// offset points forward, the other side's back, with the end caps pushed
/// `d` past the line ends. Per-vertex normals come from the neighbouring
/// points (central difference), which never spikes on sharp turns — at
/// worst the loop hugs a corner slightly closer than `d`.
fn outline_around_polyline(line: &[egui::Pos2], d: f32) -> Vec<egui::Pos2> {
    let n = line.len();
    let mut left = Vec::with_capacity(2 * n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        let t = line[(i + 1).min(n - 1)] - line[i.saturating_sub(1)];
        let len = t.length();
        if len < 1e-6 {
            continue;
        }
        let t = t / len;
        // Extend the first/last base point along the tangent so the caps
        // clear the ink tips.
        let base = match i {
            0 => line[0] - t * d,
            _ if i == n - 1 => line[i] + t * d,
            _ => line[i],
        };
        let nrm = egui::vec2(-t.y, t.x);
        left.push(base + nrm * d);
        right.push(base - nrm * d);
    }
    right.reverse();
    left.extend(right);
    left
}

/// A closed polygon's outline pushed `d` outward. Winding (shoelace sign)
/// orients the bisector-style vertex normals, which never spike; deeply
/// concave corners may sit slightly closer than `d`.
fn outline_around_polygon(poly: &[egui::Pos2], d: f32) -> Vec<egui::Pos2> {
    let n = poly.len();
    let area: f32 = (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    // In screen coords (y down) a positive shoelace sum is a clockwise
    // polygon, whose outward side is the -90° rotation of the tangent.
    let sign = if area > 0.0 { -1.0 } else { 1.0 };
    (0..n)
        .filter_map(|i| {
            let t = poly[(i + 1) % n] - poly[(i + n - 1) % n];
            let len = t.length();
            (len > 1e-6).then(|| {
                let nrm = egui::vec2(-t.y, t.x) * (sign / len);
                poly[i] + nrm * d
            })
        })
        .collect()
}

/// Device polyline → screen points, dropping consecutive (near-)duplicates:
/// duplicated points give egui's stroke tessellator degenerate normals,
/// which show up as long spike artifacts on dense, jagged paths.
fn screen_polyline(pts: &[(f32, f32)], vm: ViewMap) -> Vec<egui::Pos2> {
    let mut out: Vec<egui::Pos2> = Vec::with_capacity(pts.len());
    for &(x, y) in pts {
        let p = vm.to_screen(x, y);
        if out.last().is_none_or(|l| (*l - p).length_sq() > 0.09) {
            out.push(p);
        }
    }
    out
}

/// Device-space bounds → screen rect.
fn bounds_to_screen(b: oxygrace::render::Bounds, vm: ViewMap) -> egui::Rect {
    egui::Rect::from_min_max(vm.to_screen(b.x0, b.y0), vm.to_screen(b.x1, b.y1))
}

/// Screen-space bounding rect of an element's recorded geometry.
fn bounds_on_screen(info: &oxygrace::RenderInfo, id: oxygrace::ElementId, vm: ViewMap) -> Option<egui::Rect> {
    Some(bounds_to_screen(info.bounds(id)?, vm))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_letterboxes_and_round_trips() {
        // A wide page in a tall available rect: letterboxed vertically.
        let avail = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(400.0, 800.0));
        let vm = ViewMap::fit(avail, 792, 612);
        // Page aspect preserved.
        let aspect = vm.img_rect.width() / vm.img_rect.height();
        assert!((aspect - 792.0 / 612.0).abs() < 1e-4);
        // Fits inside and is centered.
        assert!(avail.contains_rect(vm.img_rect));
        assert!((vm.img_rect.center().x - avail.center().x).abs() < 0.5);
        assert!((vm.img_rect.center().y - avail.center().y).abs() < 0.5);
        // Corners map to (0,0) and (page_w, page_h), and round-trip.
        let (x0, y0) = vm.to_device(vm.img_rect.min);
        let (x1, y1) = vm.to_device(vm.img_rect.max);
        assert!(x0.abs() < 1e-3 && y0.abs() < 1e-3);
        assert!((x1 - 792.0).abs() < 1e-2 && (y1 - 612.0).abs() < 1e-2);
        let p = vm.to_screen(396.0, 306.0);
        let (bx, by) = vm.to_device(p);
        assert!((bx - 396.0).abs() < 1e-3 && (by - 306.0).abs() < 1e-3);
    }

    /// The highlight outline stays a fixed gap away from the ink: a
    /// horizontal line gets a closed corridor `d` around it, and a polygon
    /// outline is pushed outward regardless of its winding.
    #[test]
    fn highlight_outlines_keep_their_distance() {
        // Corridor around a horizontal 2-point line.
        let line = [egui::pos2(0.0, 5.0), egui::pos2(10.0, 5.0)];
        let out = outline_around_polyline(&line, 2.0);
        assert_eq!(out.len(), 4);
        for v in &out {
            // 2 above/below the line, caps extended 2 past the ends.
            assert!((v.y - 3.0).abs() < 1e-4 || (v.y - 7.0).abs() < 1e-4);
            assert!((-2.0..=12.0).contains(&v.x));
        }
        // A square is offset outward for both windings.
        let square = [
            egui::pos2(0.0, 0.0),
            egui::pos2(10.0, 0.0),
            egui::pos2(10.0, 10.0),
            egui::pos2(0.0, 10.0),
        ];
        for poly in [square, {
            let mut r = square;
            r.reverse();
            r
        }] {
            for v in outline_around_polygon(&poly, 2.0) {
                let inside = (0.0..=10.0).contains(&v.x) && (0.0..=10.0).contains(&v.y);
                assert!(!inside, "vertex {v:?} not pushed outward");
            }
        }
    }

    /// The set contour: distant clusters get separate loops, each tracing
    /// the isoline ~SET_GAP away; a dense run of points merges into one.
    #[test]
    fn set_contour_follows_clusters() {
        let bbox = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0));
        let (a, b) = (egui::pos2(25.0, 30.0), egui::pos2(75.0, 30.0));
        let mut grid = DistGrid::new(bbox, 3.0);
        for p in [a, b] {
            grid.stamp(p, 0.0, 6.0 + 9.0);
        }
        let loops = marching_squares(&grid, 6.0);
        assert_eq!(loops.len(), 2, "two distant points → two loops");
        for lp in &loops {
            let c = (lp.iter().fold(egui::Vec2::ZERO, |s, p| s + p.to_vec2()) / lp.len() as f32)
                .to_pos2();
            let target = if (c - a).length() < (c - b).length() { a } else { b };
            for p in lp {
                let d = (*p - target).length();
                assert!((3.0..=9.5).contains(&d), "contour should sit ~6 px out, got {d}");
            }
        }
        // A dense run of points merges into a single loop.
        let mut grid = DistGrid::new(bbox, 3.0);
        for i in 0..30 {
            grid.stamp(egui::pos2(20.0 + i as f32 * 2.0, 30.0), 0.0, 15.0);
        }
        assert_eq!(marching_squares(&grid, 6.0).len(), 1);
    }
}
