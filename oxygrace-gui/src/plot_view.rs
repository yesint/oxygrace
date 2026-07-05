//! The central plot canvas: zoom-to-fit letterboxed blit of the rendered
//! page texture, the screen ↔ device-pixel coordinate mapping, selection,
//! and direct manipulation (drag-move objects, drag-resize the viewport).

use oxygrace::render::WorldTransform;
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
        app.refocus = true;
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
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
    } else if resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
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
                let (dx, dy) = vm.to_device(pos);
                let side = app.page_size.0.min(app.page_size.1) as f64;
                let dvx = (dx - pan.start.0) as f64 / side;
                let dvy = -((dy - pan.start.1) as f64) / side;
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
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
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

/// The graph whose viewport contains device point `(dx, dy)` — topmost
/// (last-drawn) visible graph wins.
fn graph_at(app: &App, dx: f32, dy: f32) -> Option<usize> {
    let project = app.project.as_ref()?;
    let (pw, ph) = app.page_size;
    let side = pw.min(ph) as f64;
    for (i, g) in project.graphs.iter().enumerate().rev() {
        if g.hidden {
            continue;
        }
        let v = g.view;
        let x0 = (v.xmin * side) as f32;
        let x1 = (v.xmax * side) as f32;
        let y0 = (ph as f64 - v.ymax * side) as f32;
        let y1 = (ph as f64 - v.ymin * side) as f32;
        if dx >= x0 && dx <= x1 && dy >= y0 && dy <= y1 {
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
    let side = app.page_size.0.min(app.page_size.1) as f64;
    vm.to_screen((vx * side) as f32, (app.page_size.1 as f64 - vy * side) as f32)
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
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
    } else if let Some(pos) = resp.hover_pos() {
        if let Some(h) = handle_at(app, pos, vm) {
            ui.output_mut(|o| o.cursor_icon = cursor_for(h));
        } else if app.hover.is_some_and(draggable) {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
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
                app.refocus = true;
                app.drag = Some(DragState { id, kind: DragKind::Move, start: dev, orig });
            }
        }
    }

    if let Some(drag) = app.drag {
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let dev = vm.to_device(pos);
                // Device delta → view delta (view Y is up, device Y down).
                let side = app.page_size.0.min(app.page_size.1) as f64;
                let dvx = (dev.0 - drag.start.0) as f64 / side;
                let dvy = -((dev.1 - drag.start.1) as f64) / side;
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
/// symbols, label boxes) rather than one big bounding box.
fn shape_highlighted(id: oxygrace::ElementId) -> bool {
    use oxygrace::ElementId::*;
    matches!(id, Set { .. } | AxisBar { .. } | TickLabels { .. } | LineObj(_))
}

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

    // Hover highlight (skip when same as selection).
    if let Some(id) = app.hover.filter(|h| app.selection != Some(*h)) {
        highlight(&painter, info, id, vm, 0.55);
    }
    if let Some(id) = app.selection {
        highlight(&painter, info, id, vm, 1.0);
    }

    use crate::theme::{ACCENT, HALO};
    // Endpoint handles of a selected line annotation (drag affordances).
    if let Some(ElementId::LineObj(i)) = app.selection {
        if let Some((p1, p2)) = line_endpoints_screen(app, vm, i) {
            for p in [p1, p2] {
                let hr = egui::Rect::from_center_size(p, egui::vec2(10.0, 10.0));
                painter.rect_filled(hr, 1.0, HALO);
                painter.rect_stroke(hr, 1.0, egui::Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
            }
        }
    }
    // Rotate markers: circles at the corners while rotate mode is armed.
    if let Some(sel) = app.rotate_armed.filter(|s| app.selection == Some(*s)) {
        if let Some(r) = bounds_on_screen(info, sel, vm) {
            let r = r.expand(8.0);
            for p in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()] {
                painter.circle_filled(p, 6.0, HALO);
                painter.circle_stroke(p, 6.0, egui::Stroke::new(2.0, ACCENT));
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
) {
    use crate::theme::{ACCENT, HALO};
    let halo = HALO.gamma_multiply(0.8 * intensity);
    let accent = ACCENT.gamma_multiply(intensity);

    if shape_highlighted(id) {
        for (clip, shape) in info.shapes_of(id) {
            // Respect the clip the primitive was drawn under, so highlights
            // of clipped data don't stretch outside the graph viewport.
            let painter = &match clip {
                Some(c) => {
                    let r = egui::Rect::from_min_max(
                        vm.to_screen(c.x0, c.y0),
                        vm.to_screen(c.x1, c.y1),
                    );
                    painter.with_clip_rect(r.intersect(painter.clip_rect()))
                }
                None => painter.clone(),
            };
            match shape {
                oxygrace::OverlayShape::Lines { pts, half_width } => {
                    let line = screen_polyline(pts, vm);
                    if line.len() < 2 {
                        continue;
                    }
                    let w = (2.0 * half_width * vm.scale).max(1.5);
                    painter.add(egui::Shape::line(
                        line.clone(),
                        egui::Stroke::new(w + 5.0, halo),
                    ));
                    painter.add(egui::Shape::line(
                        line,
                        egui::Stroke::new(w + 2.0, accent.gamma_multiply(0.75)),
                    ));
                }
                oxygrace::OverlayShape::Polygon(pts) => {
                    let line = screen_polyline(pts, vm);
                    if line.len() < 2 {
                        continue;
                    }
                    painter.add(egui::Shape::closed_line(
                        line.clone(),
                        egui::Stroke::new(5.0, halo),
                    ));
                    painter.add(egui::Shape::closed_line(line, egui::Stroke::new(2.0, accent)));
                }
                oxygrace::OverlayShape::Rect(b) => {
                    let r = egui::Rect::from_min_max(
                        vm.to_screen(b.x0, b.y0),
                        vm.to_screen(b.x1, b.y1),
                    )
                    .expand(1.5);
                    painter.rect_stroke(r, 1.0, egui::Stroke::new(3.5, halo), egui::StrokeKind::Outside);
                    painter.rect_stroke(r, 1.0, egui::Stroke::new(1.5, accent), egui::StrokeKind::Outside);
                }
            }
        }
        return;
    }

    // Box-like elements: bounds box (+ handles when selected, not hovered).
    let Some(r) = bounds_on_screen(info, id, vm) else { return };
    let r = r.expand(3.0);
    painter.rect_stroke(r, 0.0, egui::Stroke::new(5.0, halo), egui::StrokeKind::Outside);
    painter.rect_stroke(r, 0.0, egui::Stroke::new(2.5, accent), egui::StrokeKind::Outside);
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
            painter.rect_filled(hr, 1.0, HALO);
            painter.rect_stroke(hr, 1.0, egui::Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
        }
    }
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

/// Screen-space bounding rect of an element's recorded geometry.
fn bounds_on_screen(info: &oxygrace::RenderInfo, id: oxygrace::ElementId, vm: ViewMap) -> Option<egui::Rect> {
    let b = info.bounds(id)?;
    Some(egui::Rect::from_min_max(
        vm.to_screen(b.x0, b.y0),
        vm.to_screen(b.x1, b.y1),
    ))
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
}
