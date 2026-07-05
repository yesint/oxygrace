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
    /// Filled region: hit when the point is inside (or within `tol` of an
    /// edge). Fills are opaque, so they also occlude records below them.
    Polygon(Vec<(f32, f32)>),
    /// Stroked path: hit within `half_width + tol` of any segment.
    Polyline { pts: Vec<(f32, f32)>, half_width: f32 },
    /// Bounding box (axis-aligned text, odd symbols).
    Rect(Bounds),
    /// Rotated text box (4 corners): direct-ink semantics like [`Rect`],
    /// but tight around angled text — the axis-aligned bbox of diagonal
    /// text overstates by up to √2 and steals clicks from what is under it.
    Quad(Vec<(f32, f32)>),
    /// Axis-aligned ellipse: a filled disk when `ring_half_width` is
    /// `None`, otherwise only the outline ring hits (a hollow ellipse's
    /// empty center must not capture clicks aimed through it).
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        ring_half_width: Option<f32>,
    },
}

#[derive(Debug)]
struct ElementRecord {
    id: ElementId,
    shape: RecordShape,
    /// Precomputed bbox of `shape` for the hit-test pre-filter.
    bbox: Bounds,
    /// Approximate on-screen ink area of `shape` in px² — the hit-test
    /// specificity tiebreak (at equal class and distance the smaller,
    /// more specific element wins).
    area: f32,
    /// Clip rectangle active when the primitive was drawn: a clipped-away
    /// part of a curve must not hit-test outside the graph viewport.
    clip: Option<Bounds>,
    /// An explicit click-region (plot area, legend area) rather than drawn
    /// ink — regions always lose hit-test priority to drawn elements.
    region: bool,
    /// Whether a fill polygon actually hides what is under it: translucent
    /// fills (pen alpha < 255) are see-through and must not demote the
    /// elements visible through them.
    opaque: bool,
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
    /// slack), deduplicated, ordered by how directly the click lands on
    /// each one. A GUI can cycle through the list on repeated clicks to
    /// reach the elements ranked lower.
    ///
    /// Candidates are scored, not just draw-ordered:
    /// 1. visible elements before ones hidden under an opaque fill drawn
    ///    over them (all Grace fills paint fg-on-bg, so any fill polygon
    ///    containing the point occludes everything below it);
    /// 2. by class — ink you are directly on (strokes, text, symbols),
    ///    then fills you are inside (or near the edge of), then explicit
    ///    click-regions (the plot area rectangle never shadows a curve);
    /// 3. by distance to the ink (being *on* an element beats being within
    ///    tolerance of it);
    /// 4. by ink area — the smaller, more specific element wins a tie (a
    ///    coincident axis edge beats the whole frame outline);
    /// 5. topmost draw order last, as the final tiebreak.
    pub fn hit_candidates(&self, x: f32, y: f32, tol: f32) -> Vec<ElementId> {
        struct Cand {
            id: ElementId,
            occluded: bool,
            /// 0 = direct ink, 1 = fill, 2 = click-region.
            class: u8,
            /// Distance to the ink, quantized to whole pixels so the area
            /// tiebreak still applies to "essentially equal" distances.
            dist_px: i32,
            area: f32,
            order: usize,
        }
        let mut cands: Vec<Cand> = Vec::new();
        // Top-down: newest (topmost) record first, tracking when an opaque
        // fill has covered the point — everything drawn below it is
        // invisible there and ranks behind (still reachable by cycling).
        let mut covered = false;
        for (order, r) in self.records.iter().enumerate().rev() {
            if let Some(clip) = &r.clip {
                if !clip.contains(x, y, 0.0) {
                    continue;
                }
            }
            // Pre-filter: stroked ink extends half_width beyond the bbox
            // of its centerline points.
            let slack = match &r.shape {
                RecordShape::Polyline { half_width, .. } => half_width + tol,
                RecordShape::Ellipse { ring_half_width: Some(hw), .. } => hw + tol,
                _ => tol,
            };
            if !r.bbox.contains(x, y, slack) {
                continue;
            }
            // (class, distance to visible ink); `None` = no hit.
            let mut inside_fill = false;
            let hit = match &r.shape {
                RecordShape::Rect(b) => {
                    let d = dist_to_rect(b, x, y);
                    (d <= tol).then_some((0u8, d))
                }
                RecordShape::Quad(pts) => {
                    let d = if point_in_polygon(pts, x, y) {
                        0.0
                    } else {
                        dist_to_polyline(pts, x, y, true)
                    };
                    (d <= tol).then_some((0u8, d))
                }
                RecordShape::Ellipse { cx, cy, rx, ry, ring_half_width } => {
                    let d = dist_to_ellipse(*cx, *cy, *rx, *ry, *ring_half_width, x, y);
                    (d <= tol).then_some((0u8, d))
                }
                RecordShape::Polygon(pts) => {
                    if point_in_polygon(pts, x, y) {
                        inside_fill = !r.region && r.opaque;
                        Some((1u8, 0.0))
                    } else {
                        let d = dist_to_polyline(pts, x, y, true);
                        (d <= tol).then_some((1u8, d))
                    }
                }
                RecordShape::Polyline { pts, half_width } => {
                    let d = (dist_to_polyline(pts, x, y, false) - half_width).max(0.0);
                    (d <= tol).then_some((0u8, d))
                }
            };
            if let Some((class, dist)) = hit {
                cands.push(Cand {
                    id: r.id,
                    occluded: covered,
                    class: if r.region { 2 } else { class },
                    dist_px: dist.round() as i32,
                    area: r.area,
                    order,
                });
            }
            // The covering fill itself was pushed with the *previous* state,
            // so it stays visible; only records below it are occluded.
            if inside_fill {
                covered = true;
            }
        }
        cands.sort_by(|a, b| {
            a.occluded
                .cmp(&b.occluded)
                .then(a.class.cmp(&b.class))
                .then(a.dist_px.cmp(&b.dist_px))
                .then(a.area.total_cmp(&b.area))
                .then(b.order.cmp(&a.order))
        });
        // Best-scoring record per element (the list is sorted best-first).
        let mut out: Vec<ElementId> = Vec::new();
        for c in cands {
            if !out.contains(&c.id) {
                out.push(c.id);
            }
        }
        out
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
                RecordShape::Polygon(pts) | RecordShape::Quad(pts) => OverlayShape::Polygon(pts),
                RecordShape::Rect(b) => OverlayShape::Rect(*b),
                // Highlights draw the bounding box for ellipses.
                RecordShape::Ellipse { .. } => OverlayShape::Rect(r.bbox),
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
        self.record_impl(shape, false, true);
    }

    /// Record a fill that may be translucent: only opaque fills occlude
    /// (demote) the elements drawn under them in hit-testing.
    pub(crate) fn record_fill(&mut self, shape: RecordShape, opaque: bool) {
        self.record_impl(shape, false, opaque);
    }

    /// Record an explicit clickable region (lowest hit priority).
    pub(crate) fn record_region(&mut self, shape: RecordShape) {
        self.record_impl(shape, true, true);
    }

    fn record_impl(&mut self, shape: RecordShape, region: bool, opaque: bool) {
        if self.muted > 0 {
            return;
        }
        let Some(&id) = self.stack.last() else { return };
        let bbox = match &shape {
            RecordShape::Rect(b) => *b,
            RecordShape::Polygon(pts)
            | RecordShape::Quad(pts)
            | RecordShape::Polyline { pts, .. } => match Bounds::from_points(pts) {
                Some(b) => b,
                None => return,
            },
            RecordShape::Ellipse { cx, cy, rx, ry, .. } => Bounds {
                x0: cx - rx,
                y0: cy - ry,
                x1: cx + rx,
                y1: cy + ry,
            },
        };
        let area = ink_area(&shape);
        self.info.records.push(ElementRecord {
            id,
            shape,
            bbox,
            area,
            clip: self.clip,
            region,
            opaque,
        });
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

/// Distance from `(x, y)` to the nearest segment of the polyline
/// (`closed` adds the last→first segment).
fn dist_to_polyline(pts: &[(f32, f32)], x: f32, y: f32, closed: bool) -> f32 {
    let n = pts.len();
    if n == 0 {
        return f32::INFINITY;
    }
    if n == 1 {
        return (x - pts[0].0).hypot(y - pts[0].1);
    }
    let last = if closed { n } else { n - 1 };
    let mut best = f32::INFINITY;
    for i in 0..last {
        let (x1, y1) = pts[i];
        let (x2, y2) = pts[(i + 1) % n];
        best = best.min(dist2_to_segment(x, y, x1, y1, x2, y2));
    }
    best.sqrt()
}

/// Distance from `(x, y)` to an axis-aligned rect (0 inside).
fn dist_to_rect(b: &Bounds, x: f32, y: f32) -> f32 {
    let dx = (b.x0 - x).max(x - b.x1).max(0.0);
    let dy = (b.y0 - y).max(y - b.y1).max(0.0);
    dx.hypot(dy)
}

/// Approximate on-screen ink area of a shape in px², floored to 1 so
/// degenerate shapes get no unfair specificity advantage.
fn ink_area(shape: &RecordShape) -> f32 {
    match shape {
        RecordShape::Rect(b) => ((b.x1 - b.x0) * (b.y1 - b.y0)).max(1.0),
        RecordShape::Polygon(pts) | RecordShape::Quad(pts) => polygon_area(pts).max(1.0),
        RecordShape::Polyline { pts, half_width } => {
            (polyline_len(pts) * (2.0 * half_width).max(1.0)).max(1.0)
        }
        RecordShape::Ellipse { rx, ry, ring_half_width, .. } => {
            let a = match ring_half_width {
                None => std::f32::consts::PI * rx * ry,
                // Ring ink ≈ perimeter (Ramanujan) × stroke width.
                Some(hw) => {
                    let p = std::f32::consts::PI
                        * (3.0 * (rx + ry) - ((3.0 * rx + ry) * (rx + 3.0 * ry)).sqrt());
                    p * (2.0 * hw).max(1.0)
                }
            };
            a.max(1.0)
        }
    }
}

/// Approximate distance to an ellipse: 0 inside the disk (or on the ring),
/// else the normalized radial gap scaled by the smaller semi-axis. Exact
/// for circles, adequate within hit tolerances for moderate aspect ratios.
fn dist_to_ellipse(cx: f32, cy: f32, rx: f32, ry: f32, ring: Option<f32>, x: f32, y: f32) -> f32 {
    if rx <= 0.0 || ry <= 0.0 {
        return f32::INFINITY;
    }
    let rn = ((x - cx) / rx).hypot((y - cy) / ry);
    let rmin = rx.min(ry);
    match ring {
        None => ((rn - 1.0) * rmin).max(0.0),
        Some(hw) => ((rn - 1.0).abs() * rmin - hw).max(0.0),
    }
}

/// Shoelace polygon area (absolute).
fn polygon_area(pts: &[(f32, f32)]) -> f32 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0f32;
    let mut j = n - 1;
    for i in 0..n {
        sum += (pts[j].0 + pts[i].0) * (pts[j].1 - pts[i].1);
        j = i;
    }
    (sum / 2.0).abs()
}

fn polyline_len(pts: &[(f32, f32)]) -> f32 {
    pts.windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum()
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
        assert!((dist_to_polyline(&line, 5.0, 1.5, false) - 1.5).abs() < 1e-4);
        assert!((dist_to_polyline(&line, 13.0, 0.0, false) - 3.0).abs() < 1e-4);
        assert_eq!(dist_to_polyline(&line, 5.0, 0.0, false), 0.0);
    }

    /// A fill drawn over an earlier stroke occludes it: inside the fill the
    /// fill ranks first and the hidden stroke stays reachable by cycling;
    /// on the stroke's visible part the stroke wins.
    #[test]
    fn opaque_fill_occludes_earlier_stroke() {
        let s0 = ElementId::Set { graph: 0, set: 0 };
        let s1 = ElementId::Set { graph: 0, set: 1 };
        let mut rec = Recorder::default();
        rec.push(s0);
        rec.record(RecordShape::Polyline {
            pts: vec![(0.0, 20.0), (100.0, 20.0)],
            half_width: 1.0,
        });
        rec.pop();
        // Set 1's opaque fill covers x 40..100 (drawn later).
        rec.push(s1);
        rec.record(RecordShape::Polygon(vec![
            (40.0, 0.0),
            (100.0, 0.0),
            (100.0, 40.0),
            (40.0, 40.0),
        ]));
        rec.pop();
        let info = rec.finish();
        // Visible part of the line.
        assert_eq!(info.hit_test(20.0, 20.0, 3.0), Some(s0));
        // Inside the fill: the fill first, the covered line second.
        assert_eq!(info.hit_candidates(70.0, 20.0, 3.0), vec![s1, s0]);
    }

    /// Being exactly ON a stroke beats being merely NEAR a fill's edge,
    /// even when the fill was drawn later.
    #[test]
    fn exact_stroke_beats_near_fill_edge() {
        let set = ElementId::Set { graph: 0, set: 0 };
        let bx = ElementId::BoxObj(0);
        let mut rec = Recorder::default();
        rec.push(set);
        rec.record(RecordShape::Polyline {
            pts: vec![(0.0, 20.0), (100.0, 20.0)],
            half_width: 0.5,
        });
        rec.pop();
        // A filled box drawn later, its top edge 5px below the line.
        rec.push(bx);
        rec.record(RecordShape::Polygon(vec![
            (0.0, 25.0),
            (100.0, 25.0),
            (100.0, 60.0),
            (0.0, 60.0),
        ]));
        rec.pop();
        let info = rec.finish();
        let c = info.hit_candidates(50.0, 20.0, 6.0);
        assert_eq!(c[0], set, "exact stroke hit must outrank a nearby fill edge");
        assert!(c.contains(&bx), "the box stays reachable by cycling");
        // In the middle of the box, away from the line: the box wins.
        assert_eq!(info.hit_test(50.0, 45.0, 6.0), Some(bx));
    }

    /// Coincident strokes: the shorter (more specific) ink wins the tie —
    /// an axis edge over the whole frame outline it lies on.
    #[test]
    fn smaller_ink_wins_coincident_strokes() {
        let axis = ElementId::AxisBar { graph: 0, axis: 0 };
        let frame = ElementId::Frame(0);
        let mut rec = Recorder::default();
        // Axis edge along the bottom, then the frame outline drawn on top.
        rec.push(axis);
        rec.record(RecordShape::Polyline {
            pts: vec![(0.0, 100.0), (100.0, 100.0)],
            half_width: 0.7,
        });
        rec.pop();
        rec.push(frame);
        rec.record(RecordShape::Polyline {
            pts: vec![
                (0.0, 0.0),
                (100.0, 0.0),
                (100.0, 100.0),
                (0.0, 100.0),
                (0.0, 0.0),
            ],
            half_width: 0.7,
        });
        rec.pop();
        let info = rec.finish();
        assert_eq!(info.hit_candidates(50.0, 100.0, 3.0), vec![axis, frame]);
    }

    /// A thick stroke's ink extends beyond its centerline bbox: clicks on
    /// that ink must hit even with a tolerance smaller than the width
    /// (the bbox pre-filter must use half_width + tol).
    #[test]
    fn thick_stroke_hits_beyond_centerline_bbox() {
        let set = ElementId::Set { graph: 0, set: 0 };
        let mut rec = Recorder::default();
        rec.push(set);
        rec.record(RecordShape::Polyline {
            pts: vec![(10.0, 50.0), (90.0, 50.0)],
            half_width: 8.0,
        });
        rec.pop();
        let info = rec.finish();
        // 6px above the centerline start: on the ink (half_width 8), tol 1.
        assert_eq!(info.hit_test(10.0, 44.0, 1.0), Some(set));
    }

    /// Angled text records a tight quad: points inside its axis-aligned
    /// bbox but outside the rotated box must not hit.
    #[test]
    fn quad_hits_tight_rotated_box() {
        let s = ElementId::StringObj(0);
        let mut rec = Recorder::default();
        rec.push(s);
        // A 45°-rotated box: the diamond |x−50| + |y−50| ≤ 20.
        rec.record(RecordShape::Quad(vec![
            (50.0, 30.0),
            (70.0, 50.0),
            (50.0, 70.0),
            (30.0, 50.0),
        ]));
        rec.pop();
        let info = rec.finish();
        assert_eq!(info.hit_test(50.0, 50.0, 2.0), Some(s));
        // Inside the AABB (30..70)², ~10px from the diamond: no hit.
        assert_eq!(info.hit_test(33.0, 33.0, 2.0), None);
    }

    /// Hollow ellipses hit only near their ring; disks hit anywhere inside;
    /// the bbox corner (outside the ellipse) hits neither.
    #[test]
    fn ellipse_ring_and_disk() {
        let ring_id = ElementId::EllipseObj(0);
        let disk_id = ElementId::EllipseObj(1);
        let mut rec = Recorder::default();
        rec.push(ring_id);
        rec.record(RecordShape::Ellipse {
            cx: 50.0,
            cy: 50.0,
            rx: 30.0,
            ry: 20.0,
            ring_half_width: Some(1.0),
        });
        rec.pop();
        rec.push(disk_id);
        rec.record(RecordShape::Ellipse {
            cx: 200.0,
            cy: 50.0,
            rx: 30.0,
            ry: 20.0,
            ring_half_width: None,
        });
        rec.pop();
        let info = rec.finish();
        // Ring: empty center misses, boundary hits, bbox corner misses.
        assert_eq!(info.hit_test(50.0, 50.0, 3.0), None);
        assert_eq!(info.hit_test(80.0, 50.0, 3.0), Some(ring_id));
        assert_eq!(info.hit_test(78.0, 32.0, 3.0), None);
        // Disk: center and boundary hit.
        assert_eq!(info.hit_test(200.0, 50.0, 3.0), Some(disk_id));
        assert_eq!(info.hit_test(230.0, 50.0, 3.0), Some(disk_id));
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
