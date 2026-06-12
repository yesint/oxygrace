//! Hit-test recording: an optional side-channel on the canvas that notes,
//! for every drawn primitive, which model element it belongs to and what
//! device-space region it covers. A GUI renders once with recording enabled
//! and then answers "what is under the cursor?" geometrically against the
//! collected [`RenderInfo`] — the renderer itself stays untouched (the
//! recorder is a pure observer; pixel output is identical with it on or off).

/// Identifies a selectable element of the plot model.
///
/// Indices refer to the model: `graph` into `Project::graphs`, `set` into
/// `Graph::sets`, `axis` into `Graph::axes` (0 X, 1 Y, 2 AltX, 3 AltY), and
/// object indices into the respective `Project` vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementId {
    /// The graph as a whole (the frame interior) — the fallback when a click
    /// lands inside the viewport but on no specific element.
    Graph(usize),
    /// The frame border / background fill.
    Frame(usize),
    Title(usize),
    Subtitle(usize),
    /// Axis bar, tick marks and grid lines.
    AxisBar { graph: usize, axis: usize },
    /// Tick label texts.
    TickLabels { graph: usize, axis: usize },
    AxisLabel { graph: usize, axis: usize },
    Set { graph: usize, set: usize },
    Legend(usize),
    /// Annotation objects (indices into `Project::strings` etc.).
    StringObj(usize),
    LineObj(usize),
    BoxObj(usize),
    EllipseObj(usize),
    Timestamp,
}

/// An axis-aligned rectangle in device pixels (y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Bounds {
    fn from_points(pts: &[(f32, f32)]) -> Option<Bounds> {
        let mut it = pts.iter();
        let &(x, y) = it.next()?;
        let mut b = Bounds { x0: x, y0: y, x1: x, y1: y };
        for &(x, y) in it {
            b.x0 = b.x0.min(x);
            b.y0 = b.y0.min(y);
            b.x1 = b.x1.max(x);
            b.y1 = b.y1.max(y);
        }
        Some(b)
    }

    fn contains(&self, x: f32, y: f32, tol: f32) -> bool {
        x >= self.x0 - tol && x <= self.x1 + tol && y >= self.y0 - tol && y <= self.y1 + tol
    }

    fn union(self, o: Bounds) -> Bounds {
        Bounds {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
}

/// A view of one recorded primitive's device-space geometry, for drawing
/// selection highlights over the element's actual representation.
#[derive(Debug, Clone, Copy)]
pub enum OverlayShape<'a> {
    Lines { pts: &'a [(f32, f32)], half_width: f32 },
    Polygon(&'a [(f32, f32)]),
    Rect(Bounds),
}

/// The device-space geometry recorded for one primitive.
#[derive(Debug, Clone)]
pub(crate) enum RecordShape {
    /// Filled region: hit when the point is inside (or within `tol` of an edge).
    Polygon(Vec<(f32, f32)>),
    /// Stroked path: hit within `max(half_width, tol)` of any segment.
    Polyline { pts: Vec<(f32, f32)>, half_width: f32 },
    /// Bounding box only (text, symbols, circles/ellipses).
    Rect(Bounds),
}

#[derive(Debug)]
struct ElementRecord {
    id: ElementId,
    shape: RecordShape,
    /// Precomputed bbox of `shape` for the hit-test pre-filter.
    bbox: Bounds,
    /// Clip rectangle active when the primitive was drawn: a clipped-away
    /// part of a curve must not hit-test outside the graph viewport.
    clip: Option<Bounds>,
    /// An explicit click-region (plot area, legend area) rather than drawn
    /// ink — regions always lose hit-test priority to drawn elements.
    region: bool,
}

/// Element geometry collected during one render pass.
#[derive(Debug, Default)]
pub struct RenderInfo {
    records: Vec<ElementRecord>,
}

impl RenderInfo {
    /// The element under device point `(x, y)` — the first of
    /// [`RenderInfo::hit_candidates`].
    pub fn hit_test(&self, x: f32, y: f32, tol: f32) -> Option<ElementId> {
        self.hit_candidates(x, y, tol).first().copied()
    }

    /// Every element under device point `(x, y)` (with `tol` pixels of
    /// slack), deduplicated, ordered top-down by draw order — with two
    /// adjustments matching what a user means by "what I clicked":
    /// drawn ink always beats explicit click-regions (the plot-area
    /// rectangle never shadows an axis or a curve), and an axis bar is
    /// promoted above the frame border it usually coincides with. A GUI can
    /// cycle through the list on repeated clicks to reach occluded elements.
    pub fn hit_candidates(&self, x: f32, y: f32, tol: f32) -> Vec<ElementId> {
        let mut ink: Vec<ElementId> = Vec::new();
        let mut regions: Vec<ElementId> = Vec::new();
        for r in self.records.iter().rev() {
            if ink.contains(&r.id) || regions.contains(&r.id) {
                continue;
            }
            if let Some(clip) = &r.clip {
                if !clip.contains(x, y, 0.0) {
                    continue;
                }
            }
            if !r.bbox.contains(x, y, tol) {
                continue;
            }
            let hit = match &r.shape {
                RecordShape::Rect(b) => b.contains(x, y, tol),
                RecordShape::Polygon(pts) => {
                    point_in_polygon(pts, x, y) || near_polyline(pts, x, y, tol, true)
                }
                RecordShape::Polyline { pts, half_width } => {
                    near_polyline(pts, x, y, half_width.max(tol), false)
                }
            };
            if hit {
                if r.region {
                    regions.push(r.id);
                } else {
                    ink.push(r.id);
                }
            }
        }
        // Promote the first axis bar above a leading frame hit.
        if let Some(axis_pos) = ink
            .iter()
            .position(|id| matches!(id, ElementId::AxisBar { .. }))
        {
            if axis_pos > 0 && ink[..axis_pos].iter().all(|id| matches!(id, ElementId::Frame(_))) {
                ink[..=axis_pos].rotate_right(1);
            }
        }
        // Regions last; drop region entries whose id already hit as ink.
        for id in regions {
            if !ink.contains(&id) {
                ink.push(id);
            }
        }
        ink
    }

    /// The recorded device-space geometry of an element, in draw order, with
    /// the clip rectangle each primitive was drawn under — lets a GUI
    /// highlight the element's actual (clipped) representation rather than
    /// its bounding box.
    pub fn shapes_of(
        &self,
        id: ElementId,
    ) -> impl Iterator<Item = (Option<Bounds>, OverlayShape<'_>)> {
        self.records.iter().filter(move |r| r.id == id).map(|r| {
            let shape = match &r.shape {
                RecordShape::Polyline { pts, half_width } => OverlayShape::Lines {
                    pts,
                    half_width: *half_width,
                },
                RecordShape::Polygon(pts) => OverlayShape::Polygon(pts),
                RecordShape::Rect(b) => OverlayShape::Rect(*b),
            };
            (r.clip, shape)
        })
    }

    /// Union bounding box of everything recorded for `id` (for selection
    /// handles), or `None` if the element drew nothing.
    pub fn bounds(&self, id: ElementId) -> Option<Bounds> {
        self.records
            .iter()
            .filter(|r| r.id == id)
            .map(|r| r.bbox)
            .reduce(Bounds::union)
    }

    /// All element ids that recorded at least one primitive, in draw order.
    pub fn elements(&self) -> impl Iterator<Item = ElementId> + '_ {
        let mut seen = std::collections::HashSet::new();
        self.records.iter().filter_map(move |r| seen.insert(r.id).then_some(r.id))
    }

    /// Number of recorded primitives (diagnostics/tests).
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Accumulates element records during a render pass (owned by the canvas).
#[derive(Debug, Default)]
pub(crate) struct Recorder {
    stack: Vec<ElementId>,
    info: RenderInfo,
    /// Device-space mirror of the canvas clip rectangle.
    clip: Option<Bounds>,
    /// While > 0, nothing is recorded (pure decoration like grid lines —
    /// they cover the whole plot and must not steal hovers/clicks).
    muted: u32,
}

impl Recorder {
    pub(crate) fn push(&mut self, id: ElementId) {
        self.stack.push(id);
    }

    pub(crate) fn pop(&mut self) {
        debug_assert!(!self.stack.is_empty(), "pop_element without push_element");
        self.stack.pop();
    }

    pub(crate) fn mute(&mut self) {
        self.muted += 1;
    }

    pub(crate) fn unmute(&mut self) {
        debug_assert!(self.muted > 0, "unmute without mute");
        self.muted = self.muted.saturating_sub(1);
    }

    pub(crate) fn set_clip(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.clip = Some(Bounds { x0, y0, x1, y1 });
    }

    pub(crate) fn clear_clip(&mut self) {
        self.clip = None;
    }

    /// Record a primitive under the innermost open element (drops it when no
    /// element is open — e.g. decorative output nobody can select).
    pub(crate) fn record(&mut self, shape: RecordShape) {
        self.record_impl(shape, false);
    }

    /// Record an explicit clickable region (lowest hit priority).
    pub(crate) fn record_region(&mut self, shape: RecordShape) {
        self.record_impl(shape, true);
    }

    fn record_impl(&mut self, shape: RecordShape, region: bool) {
        if self.muted > 0 {
            return;
        }
        let Some(&id) = self.stack.last() else { return };
        let bbox = match &shape {
            RecordShape::Rect(b) => *b,
            RecordShape::Polygon(pts) | RecordShape::Polyline { pts, .. } => {
                match Bounds::from_points(pts) {
                    Some(b) => b,
                    None => return,
                }
            }
        };
        self.info.records.push(ElementRecord { id, shape, bbox, clip: self.clip, region });
    }

    pub(crate) fn finish(mut self) -> RenderInfo {
        debug_assert!(self.stack.is_empty(), "unbalanced push_element/pop_element");
        self.stack.clear();
        self.info
    }
}

/// Even-odd ray-cast point-in-polygon test.
fn point_in_polygon(pts: &[(f32, f32)], x: f32, y: f32) -> bool {
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// True when `(x, y)` lies within `dist` of any segment of the polyline
/// (`closed` adds the last→first segment).
fn near_polyline(pts: &[(f32, f32)], x: f32, y: f32, dist: f32, closed: bool) -> bool {
    let d2 = dist * dist;
    let n = pts.len();
    if n == 0 {
        return false;
    }
    if n == 1 {
        let (dx, dy) = (x - pts[0].0, y - pts[0].1);
        return dx * dx + dy * dy <= d2;
    }
    let last = if closed { n } else { n - 1 };
    for i in 0..last {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % n];
        if dist2_to_segment(x, y, x1, y1, x2, y2) <= d2 {
            return true;
        }
    }
    false
}

/// Squared distance from a point to a segment.
fn dist2_to_segment(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let (vx, vy) = (x2 - x1, y2 - y1);
    let (wx, wy) = (px - x1, py - y1);
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 0.0 { ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (dx, dy) = (wx - t * vx, wy - t * vy);
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_hit() {
        let sq = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(point_in_polygon(&sq, 5.0, 5.0));
        assert!(!point_in_polygon(&sq, 15.0, 5.0));
    }

    #[test]
    fn polyline_distance() {
        let line = vec![(0.0, 0.0), (10.0, 0.0)];
        assert!(near_polyline(&line, 5.0, 1.5, 2.0, false));
        assert!(!near_polyline(&line, 5.0, 3.0, 2.0, false));
        assert!(!near_polyline(&line, 13.0, 0.0, 2.0, false));
    }

    #[test]
    fn topmost_wins_and_clip_applies() {
        let mut rec = Recorder::default();
        rec.push(ElementId::Graph(0));
        rec.record(RecordShape::Rect(Bounds { x0: 0.0, y0: 0.0, x1: 100.0, y1: 100.0 }));
        rec.push(ElementId::Set { graph: 0, set: 0 });
        rec.set_clip(0.0, 0.0, 50.0, 50.0);
        rec.record(RecordShape::Polyline {
            pts: vec![(0.0, 20.0), (100.0, 20.0)],
            half_width: 1.0,
        });
        rec.clear_clip();
        rec.pop();
        rec.pop();
        let info = rec.finish();
        // On the line, inside the clip: the set wins over the graph rect.
        assert_eq!(info.hit_test(25.0, 20.0, 2.0), Some(ElementId::Set { graph: 0, set: 0 }));
        // On the line but clipped away: falls through to the graph.
        assert_eq!(info.hit_test(75.0, 20.0, 2.0), Some(ElementId::Graph(0)));
        // Outside everything.
        assert_eq!(info.hit_test(200.0, 200.0, 2.0), None);
    }
}
