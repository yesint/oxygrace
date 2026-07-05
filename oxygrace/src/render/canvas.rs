//! The drawing canvas: the device primitives the draw layer calls
//! (polylines, filled polygons, text), all taking **view** coordinates and
//! color *indices*, which are resolved against the project's color map here.
//!
//! The canvas owns the shared geometry work — view→device mapping, dash
//! tables, pattern selection, text layout and justification — and hands the
//! resulting device-space paths to one of two backends: a raster backend
//! (tiny-skia, PNG output) or a vector backend (SVG markup). Both receive
//! identical geometry, so the SVG output matches the PNG rendering exactly,
//! with text emitted as glyph outline paths.

use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pattern, Pixmap,
    SpreadMode, Stroke, StrokeDash, Transform,
};

use crate::color::{self, Rgba};
use crate::font::FontSet;
use crate::model::Project;
use crate::patterns::PATTERN_BITS;
use crate::render::record::{Bounds, ElementId, RecordShape, Recorder, RenderInfo};
use crate::render::svg::SvgBackend;
use crate::render::transform::PageTransform;
use crate::text;

/// What a path is filled with (colors already resolved).
pub(crate) enum FillPaint {
    Solid(Rgba),
    /// A Grace hatch pattern: 16x16 tile of `fg` bits over an opaque `bg`
    /// (Grace pattern fills occlude what is below them, like the gd driver).
    Hatch { pattern: i32, fg: Rgba, bg: Rgba },
}

/// Build a 16x16 RGBA tile for a Grace fill pattern (raster backend).
fn pattern_tile(pattern: i32, color: Rgba, bg: Rgba) -> Option<Pixmap> {
    let idx = pattern as usize;
    if !(0..PATTERN_BITS.len()).contains(&idx) {
        return None;
    }
    let bits = &PATTERN_BITS[idx];
    let mut tile = Pixmap::new(16, 16)?;
    let px = tile.pixels_mut();
    for row in 0..16 {
        for col in 0..16 {
            // LSB-first within each byte (X11 bitmap order).
            let byte = bits[row * 2 + col / 8];
            let c = if (byte >> (col % 8)) & 1 == 1 { color } else { bg };
            // Colors may carry pen alpha (translucent fills): premultiply.
            px[row * 16 + col] = tiny_skia::ColorU8::from_rgba(c.r, c.g, c.b, c.a).premultiply();
        }
    }
    Some(tile)
}

/// Raster output: a tiny-skia pixmap plus the active clip mask.
struct RasterBackend {
    pixmap: Pixmap,
    /// Active clip region (device-space mask), or `None` for unclipped
    /// drawing. Mirrors Grace's `setclipping` + per-graph `viewport` clip in
    /// `draw.cpp` (`clip_line`/`clip_polygon`): data elements are clipped to
    /// the graph viewport, decorations are not.
    clip: Option<Mask>,
}

impl RasterBackend {
    fn new(width: u32, height: u32) -> Self {
        let mut pixmap = Pixmap::new(width, height).expect("non-zero page dimensions");
        pixmap.fill(tiny_skia::Color::WHITE);
        RasterBackend { pixmap, clip: None }
    }

    fn set_clip(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let mut mask = Mask::new(self.pixmap.width(), self.pixmap.height())
            .expect("non-zero page dimensions");
        if let Some(rect) = tiny_skia::Rect::from_ltrb(x0, y0, x1, y1) {
            let path = PathBuilder::from_rect(rect);
            mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        }
        self.clip = Some(mask);
    }

    fn stroke_path(&mut self, path: &Path, color: Rgba, width: f32, dash: Option<&[f32]>) {
        let mut paint = Paint::default();
        paint.set_color(color.to_skia());
        paint.anti_alias = true;
        let mut stroke = Stroke {
            width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            ..Stroke::default()
        };
        if let Some(dash) = dash {
            stroke.dash = StrokeDash::new(dash.to_vec(), 0.0);
        }
        self.pixmap
            .stroke_path(path, &paint, &stroke, Transform::identity(), self.clip.as_ref());
    }

    fn fill_path(&mut self, path: &Path, fill: &FillPaint, rule: FillRule) {
        let mut paint = Paint {
            anti_alias: true,
            ..Default::default()
        };
        match fill {
            FillPaint::Solid(c) => paint.set_color(c.to_skia()),
            FillPaint::Hatch { pattern, fg, bg } => {
                match pattern_tile(*pattern, *fg, *bg) {
                    Some(tile) => {
                        paint.shader = Pattern::new(
                            tile.as_ref(),
                            SpreadMode::Repeat,
                            FilterQuality::Nearest,
                            1.0,
                            Transform::identity(),
                        );
                        self.pixmap.fill_path(
                            path,
                            &paint,
                            rule,
                            Transform::identity(),
                            self.clip.as_ref(),
                        );
                        return;
                    }
                    // Unknown pattern -> fall back to solid foreground.
                    None => paint.set_color(fg.to_skia()),
                }
            }
        }
        self.pixmap
            .fill_path(path, &paint, rule, Transform::identity(), self.clip.as_ref());
    }
}

/// The output device a canvas draws into.
enum Backend {
    Raster(RasterBackend),
    Svg(SvgBackend),
}

impl Backend {
    fn set_clip(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        match self {
            Backend::Raster(b) => b.set_clip(x0, y0, x1, y1),
            Backend::Svg(b) => b.set_clip(x0, y0, x1, y1),
        }
    }

    fn clear_clip(&mut self) {
        match self {
            Backend::Raster(b) => b.clip = None,
            Backend::Svg(b) => b.clear_clip(),
        }
    }

    fn stroke_path(&mut self, path: &Path, color: Rgba, width: f32, dash: Option<&[f32]>) {
        match self {
            Backend::Raster(b) => b.stroke_path(path, color, width, dash),
            Backend::Svg(b) => b.stroke_path(path, color, width, dash),
        }
    }

    fn fill_path(&mut self, path: &Path, fill: &FillPaint, rule: FillRule) {
        match self {
            Backend::Raster(b) => b.fill_path(path, fill, rule),
            Backend::Svg(b) => b.fill_path(path, fill, rule),
        }
    }
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

/// Vertical text alignment relative to the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Baseline,
    Bottom,
    Middle,
    Top,
}

/// A point in view coordinates.
#[derive(Debug, Clone, Copy)]
pub struct VPoint {
    pub x: f64,
    pub y: f64,
}

/// Draws device primitives into a backend; resolves colors via the borrowed
/// [`Project`] and outlines glyphs via the borrowed [`FontSet`].
pub struct Canvas<'a> {
    backend: Backend,
    page: PageTransform,
    project: &'a Project,
    fonts: &'a FontSet,
    /// Current drawing opacity 0..=255, applied to every resolved color —
    /// Grace pens carry an alpha in QtGrace (`draw.cpp` `setalpha`), and
    /// the Qt driver stamps it onto the paint color
    /// (`x11drv.cpp` `col.setAlpha(getalpha())`).
    alpha: u8,
    /// Optional hit-test recorder (see [`crate::render::record`]). A pure
    /// observer: drawing output is identical whether it is on or off.
    recorder: Option<Recorder>,
}

impl<'a> Canvas<'a> {
    /// Create a raster (PNG) canvas with a white page sized from the project.
    pub fn new(project: &'a Project, fonts: &'a FontSet) -> Self {
        Canvas {
            backend: Backend::Raster(RasterBackend::new(project.page_width, project.page_height)),
            page: PageTransform::new(project.page_width, project.page_height),
            project,
            fonts,
            alpha: 255,
            recorder: None,
        }
    }

    /// Create a raster canvas that also records element geometry for
    /// hit-testing; finish with [`Canvas::into_pixmap`].
    pub fn new_recording(project: &'a Project, fonts: &'a FontSet) -> Self {
        Canvas {
            recorder: Some(Recorder::default()),
            ..Canvas::new(project, fonts)
        }
    }

    /// Create an SVG canvas sized from the project.
    pub fn new_svg(project: &'a Project, fonts: &'a FontSet) -> Self {
        Canvas {
            backend: Backend::Svg(SvgBackend::new(project.page_width, project.page_height)),
            page: PageTransform::new(project.page_width, project.page_height),
            project,
            fonts,
            alpha: 255,
            recorder: None,
        }
    }

    /// Set the drawing opacity for subsequent primitives (0..=255; values
    /// outside the range reset to opaque, like QtGrace's `setalpha`,
    /// `draw.cpp`). Callers pair it with a `set_alpha(255)` restore.
    pub fn set_alpha(&mut self, alpha: i32) {
        self.alpha = if (0..=255).contains(&alpha) { alpha as u8 } else { 255 };
    }

    /// Resolve a color index with the current drawing opacity stamped in
    /// (the map holds opaque RGB; alpha comes from the active pen, like
    /// QtGrace's `col.setAlpha(getalpha())`).
    fn resolve(&self, color: i32) -> Rgba {
        let mut c = color::resolve(self.project, color);
        c.a = self.alpha;
        c
    }

    /// Open an element: subsequent primitives are recorded under `id` (the
    /// innermost open element wins). No-op without a recorder.
    pub fn push_element(&mut self, id: ElementId) {
        if let Some(r) = &mut self.recorder {
            r.push(id);
        }
    }

    /// Close the innermost open element.
    pub fn pop_element(&mut self) {
        if let Some(r) = &mut self.recorder {
            r.pop();
        }
    }

    /// Suspend hit-test recording for pure decoration (e.g. grid lines,
    /// which span the whole plot and must not steal hovers/clicks).
    /// Pair with [`Canvas::unmute_recording`].
    pub fn mute_recording(&mut self) {
        if let Some(r) = &mut self.recorder {
            r.mute();
        }
    }

    pub fn unmute_recording(&mut self) {
        if let Some(r) = &mut self.recorder {
            r.unmute();
        }
    }

    /// Record an explicit clickable region (view coords) for the current
    /// element without drawing anything — e.g. the graph viewport as the
    /// click-on-empty-plot fallback, or the legend's overall box. Regions
    /// always lose hit-test priority to drawn ink.
    pub fn record_rect_view(&mut self, xmin: f64, ymin: f64, xmax: f64, ymax: f64) {
        if let Some(r) = &mut self.recorder {
            let (x0, y0) = self.page.view_to_device(xmin, ymax);
            let (x1, y1) = self.page.view_to_device(xmax, ymin);
            r.record_region(RecordShape::Rect(Bounds { x0, y0, x1, y1 }));
        }
    }

    fn record(&mut self, shape: RecordShape) {
        if let Some(r) = &mut self.recorder {
            r.record(shape);
        }
    }

    /// Record a clickable polyline (view coords) for the current element
    /// without drawing anything — e.g. the frame edges doubling as axis
    /// lines. Unlike [`Canvas::record_rect_view`] this records *ink*, so it
    /// keeps normal hit-test priority.
    pub fn record_polyline_view(&mut self, pts: &[VPoint], linewidth: f64) {
        if self.recorder.is_some() {
            let dev: Vec<(f32, f32)> = pts
                .iter()
                .map(|p| self.page.view_to_device(p.x, p.y))
                .collect();
            let half_width = (self.page.linewidth_px(linewidth) / 2.0).max(0.5);
            self.record(RecordShape::Polyline { pts: dev, half_width });
        }
    }

    /// Clip subsequent drawing to a rectangle in view coordinates, expanded by
    /// Grace's `VP_EPSILON = 0.0001` slack (`draw.cpp`). Pass the graph
    /// viewport before drawing data; call [`Canvas::clear_clip`] afterwards.
    pub fn set_clip_view(&mut self, xmin: f64, ymin: f64, xmax: f64, ymax: f64) {
        const VP_EPSILON: f64 = 0.0001; // draw.cpp:799
        let (x0, y0) = self
            .page
            .view_to_device(xmin - VP_EPSILON, ymax + VP_EPSILON);
        let (x1, y1) = self
            .page
            .view_to_device(xmax + VP_EPSILON, ymin - VP_EPSILON);
        self.backend.set_clip(x0, y0, x1, y1);
        if let Some(r) = &mut self.recorder {
            r.set_clip(x0, y0, x1, y1);
        }
    }

    /// Disable clipping (Grace `setclipping(FALSE)`).
    pub fn clear_clip(&mut self) {
        self.backend.clear_clip();
        if let Some(r) = &mut self.recorder {
            r.clear_clip();
        }
    }

    /// Access the page transform (for size conversions in the draw layer).
    pub fn page(&self) -> &PageTransform {
        &self.page
    }

    /// Encode the rendering as PNG bytes (raster canvases only).
    pub fn to_png(&self) -> Vec<u8> {
        match &self.backend {
            Backend::Raster(b) => b.pixmap.encode_png().expect("PNG encoding"),
            Backend::Svg(_) => panic!("to_png called on an SVG canvas"),
        }
    }

    /// Finish an SVG canvas and return the document (SVG canvases only).
    pub fn into_svg(self) -> String {
        match self.backend {
            Backend::Svg(b) => b.finish(),
            Backend::Raster(_) => panic!("into_svg called on a raster canvas"),
        }
    }

    /// Finish a raster canvas: the raw premultiplied-RGBA pixmap plus the
    /// recorded element geometry (empty without recording).
    pub fn into_pixmap(self) -> (Pixmap, RenderInfo) {
        match self.backend {
            Backend::Raster(b) => (
                b.pixmap,
                self.recorder.map(Recorder::finish).unwrap_or_default(),
            ),
            Backend::Svg(_) => panic!("into_pixmap called on an SVG canvas"),
        }
    }

    /// Resolve a fill (color index + Grace pattern index) for the backend.
    /// Pattern 0 means "no fill" and is handled by the callers.
    fn fill_paint(&self, color: i32, pattern: i32) -> FillPaint {
        let rgba = self.resolve(color);
        if pattern == 1 || !(0..PATTERN_BITS.len() as i32).contains(&pattern) {
            FillPaint::Solid(rgba)
        } else {
            FillPaint::Hatch {
                pattern,
                fg: rgba,
                bg: self.resolve(0),
            }
        }
    }

    /// Stroke a polyline given in view coordinates.
    pub fn draw_polyline(&mut self, pts: &[VPoint], color: i32, linewidth: f64, linestyle: i32) {
        if pts.len() < 2 || linestyle == 0 {
            return;
        }
        let mut dev: Vec<(f32, f32)> = pts
            .iter()
            .map(|p| self.page.view_to_device(p.x, p.y))
            .collect();
        // Pathologically dense solid polylines (≫ points per pixel column)
        // are reduced with M4 aggregation — first/min/max/last per device
        // x-column — which draws the same per-column envelope a thin stroke
        // would. Applied to the shared device geometry, so raster, SVG and
        // hit-recording stay consistent. Dashed lines are exempt (their
        // pattern phase depends on true path length).
        if dev.len() > DENSE_POLYLINE_LIMIT && linestyle == 1 {
            dev = m4_decimate(&dev);
        }
        let mut pb = PathBuilder::new();
        for (i, &(x, y)) in dev.iter().enumerate() {
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        let Some(path) = pb.finish() else { return };
        let width = self.page.linewidth_px(linewidth).max(0.1);
        let dash = dash_pattern(linestyle, width);
        let rgba = self.resolve(color);
        self.backend.stroke_path(&path, rgba, width, dash.as_deref());
        if self.recorder.is_some() {
            self.record(RecordShape::Polyline { pts: dev, half_width: width / 2.0 });
        }
    }

    /// Fill a closed polygon (view coords) with a color index and fill pattern.
    /// Pattern 0 = no fill, 1 = solid, 2..=31 = a tiled hatch in `color`.
    pub fn fill_polygon(&mut self, pts: &[VPoint], color: i32, pattern: i32) {
        self.fill_polygon_rule(pts, color, pattern, 0);
    }

    /// Like [`Canvas::fill_polygon`] with an explicit fill rule
    /// (0 = winding, 1 = even-odd; Grace `setfillrule` in `drawsetfill`).
    pub fn fill_polygon_rule(&mut self, pts: &[VPoint], color: i32, pattern: i32, rule: i32) {
        if pts.len() < 3 || pattern == 0 {
            return;
        }
        let dev: Vec<(f32, f32)> = pts
            .iter()
            .map(|p| self.page.view_to_device(p.x, p.y))
            .collect();
        let mut pb = PathBuilder::new();
        for (i, &(x, y)) in dev.iter().enumerate() {
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        let Some(path) = pb.finish() else { return };
        if let Some(r) = &mut self.recorder {
            // Translucent fills are see-through: they must not occlude the
            // elements visible through them in hit-testing.
            r.record_fill(RecordShape::Polygon(dev), self.alpha == 255);
        }
        let rule = if rule == 1 {
            FillRule::EvenOdd
        } else {
            FillRule::Winding
        };
        let paint = self.fill_paint(color, pattern);
        self.backend.fill_path(&path, &paint, rule);
    }

    /// Fill a path with a color index and fill pattern (shared by all fills).
    fn fill_path_pen(&mut self, path: &Path, color: i32, pattern: i32) {
        if pattern == 0 {
            return;
        }
        let paint = self.fill_paint(color, pattern);
        self.backend.fill_path(path, &paint, FillRule::Winding);
    }

    /// Width of a marked-up string in view units, at the given char size.
    pub fn text_width_view(&self, s: &str, charsize: f64, font: i32) -> f64 {
        let em = text::measure(self.fonts, s, font, &self.project.font_map) as f64;
        // em units -> view: one em at `charsize` spans charsize*MAGIC_FONT_SCALE.
        em * charsize * crate::render::transform::MAGIC_FONT_SCALE
    }

    /// One em height in view units at the given char size.
    pub fn em_view(&self, charsize: f64) -> f64 {
        charsize * crate::render::transform::MAGIC_FONT_SCALE
    }

    /// Rendered ink bounding box of a marked-up string in view units:
    /// `(x_min, y_min, x_max, y_max)`, baseline-left origin, Y up. Empty
    /// strings give a zero box. Built from the positioned glyph outlines,
    /// mirroring Grace's `update_bbox`.
    pub fn text_bbox_view(&self, s: &str, charsize: f64, font: i32) -> (f64, f64, f64, f64) {
        match text::bbox(self.fonts, s, font, &self.project.font_map) {
            Some((x0, y0, x1, y1)) => {
                let e = self.em_view(charsize);
                (x0 as f64 * e, y0 as f64 * e, x1 as f64 * e, y1 as f64 * e)
            }
            None => (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Text line height (ascent − descent) in view units. Grace accumulates
    /// tick-label bounding boxes using full font metrics (the em box, including
    /// descent/leading), so axis-label placement uses this, not the tight
    /// glyph extent.
    pub fn text_height_view(&self, charsize: f64, font: i32) -> f64 {
        (self.fonts.ascent(font) as f64 - self.fonts.descent(font) as f64) * self.em_view(charsize)
    }

    /// Fill a circle (center + radius in view units) with a color and pattern.
    pub fn fill_circle(&mut self, center: VPoint, radius_view: f64, color: i32, pattern: i32) {
        let (cx, cy) = self.page.view_to_device(center.x, center.y);
        let r = self.page.view_len_px(radius_view);
        if r <= 0.0 {
            return;
        }
        let Some(path) = PathBuilder::from_circle(cx, cy, r) else {
            return;
        };
        self.fill_path_pen(&path, color, pattern);
        self.record(RecordShape::Ellipse {
            cx,
            cy,
            rx: r,
            ry: r,
            ring_half_width: None,
        });
    }

    /// Stroke a circle outline (center + radius in view units).
    pub fn stroke_circle(&mut self, center: VPoint, radius_view: f64, color: i32, lw: f64, ls: i32) {
        if ls == 0 {
            return;
        }
        let (cx, cy) = self.page.view_to_device(center.x, center.y);
        let r = self.page.view_len_px(radius_view);
        if r <= 0.0 {
            return;
        }
        let Some(path) = PathBuilder::from_circle(cx, cy, r) else {
            return;
        };
        let width = self.page.linewidth_px(lw).max(0.1);
        let dash = dash_pattern(ls, width);
        let rgba = self.resolve(color);
        self.backend.stroke_path(&path, rgba, width, dash.as_deref());
        // The ring: a hollow circle's center doesn't capture clicks (small
        // symbols stay clickable in the middle through the hit tolerance).
        self.record(RecordShape::Ellipse {
            cx,
            cy,
            rx: r,
            ry: r,
            ring_half_width: Some(width / 2.0),
        });
    }

    /// Build the oval path inscribed in a view-coordinate rectangle.
    fn oval_path(&self, p1: VPoint, p2: VPoint) -> Option<Path> {
        let (x1, y1) = self.page.view_to_device(p1.x, p1.y);
        let (x2, y2) = self.page.view_to_device(p2.x, p2.y);
        let rect = tiny_skia::Rect::from_ltrb(x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2))?;
        PathBuilder::from_oval(rect)
    }

    /// Record the ellipse inscribed in a view rectangle: a disk, or only the
    /// outline ring when `ring_half_width` is set — a hollow ellipse's empty
    /// center must not capture clicks aimed at elements visible through it.
    fn record_ellipse(&mut self, p1: VPoint, p2: VPoint, ring_half_width: Option<f32>) {
        if self.recorder.is_none() {
            return;
        }
        let (x1, y1) = self.page.view_to_device(p1.x, p1.y);
        let (x2, y2) = self.page.view_to_device(p2.x, p2.y);
        self.record(RecordShape::Ellipse {
            cx: (x1 + x2) / 2.0,
            cy: (y1 + y2) / 2.0,
            rx: (x2 - x1).abs() / 2.0,
            ry: (y2 - y1).abs() / 2.0,
            ring_half_width,
        });
    }

    /// Fill the ellipse inscribed in the rectangle spanned by two view points.
    pub fn fill_ellipse(&mut self, p1: VPoint, p2: VPoint, color: i32, pattern: i32) {
        let Some(path) = self.oval_path(p1, p2) else { return };
        self.fill_path_pen(&path, color, pattern);
        self.record_ellipse(p1, p2, None);
    }

    /// Stroke the ellipse inscribed in the rectangle spanned by two view points.
    pub fn stroke_ellipse(&mut self, p1: VPoint, p2: VPoint, color: i32, lw: f64, ls: i32) {
        if ls == 0 {
            return;
        }
        let Some(path) = self.oval_path(p1, p2) else { return };
        let width = self.page.linewidth_px(lw).max(0.1);
        let dash = dash_pattern(ls, width);
        let rgba = self.resolve(color);
        self.backend.stroke_path(&path, rgba, width, dash.as_deref());
        self.record_ellipse(p1, p2, Some(width / 2.0));
    }

    /// Draw a marked-up string anchored at a view point.
    ///
    /// `charsize` is the Grace character size; `base_font`/`color` are the
    /// defaults for runs that do not override them. `angle` is in degrees,
    /// counter-clockwise. Alignment positions the anchor within the text box.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &mut self,
        anchor: VPoint,
        s: &str,
        charsize: f64,
        base_font: i32,
        color: i32,
        halign: HAlign,
        valign: VAlign,
        angle: f64,
    ) {
        if s.is_empty() {
            return;
        }
        let layout = text::layout(self.fonts, s, base_font, &self.project.font_map);
        if layout.glyphs.is_empty() {
            return;
        }
        let em_px = self.page.fontsize_px(charsize);
        let (ax, ay) = self.page.view_to_device(anchor.x, anchor.y);
        let theta = angle.to_radians() as f32;
        let (sin, cos) = (theta.sin(), theta.cos());
        let rot = |x: f32, y: f32| (x * cos - y * sin, x * sin + y * cos);

        // Axis-aligned bounding box of the *rotated* glyph outlines, in em
        // units (Y up) — Grace's `bbox_ll`/`bbox_ur` accumulated in WriteString.
        // The unrotated box is accumulated alongside: its rotated corners are
        // the tight hit-test quad for angled text.
        let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let (mut ux0, mut uy0, mut ux1, mut uy1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut any = false;
        for g in &layout.glyphs {
            if let Some((gx0, gy0, gx1, gy1)) = self.fonts.glyph_bbox(g.font, g.ch) {
                for &(cx, cy) in &[(gx0, gy0), (gx1, gy0), (gx1, gy1), (gx0, gy1)] {
                    let (mx, my) = g.tm.apply(cx, cy);
                    let (px, py) = (g.x + mx, g.y + my);
                    ux0 = ux0.min(px);
                    uy0 = uy0.min(py);
                    ux1 = ux1.max(px);
                    uy1 = uy1.max(py);
                    let (rx, ry) = rot(px, py);
                    bx0 = bx0.min(rx);
                    by0 = by0.min(ry);
                    bx1 = bx1.max(rx);
                    by1 = by1.max(ry);
                    any = true;
                }
            }
        }
        if !any {
            return;
        }

        // The (hfudge, vfudge) fraction of the bbox lands on the anchor
        // (Grace's `offset = bbox_ll + fudge*(bbox_ur - bbox_ll) - vp`).
        // JUST_BLINE positions by the rotated baseline instead of the bbox.
        let hfudge = match halign {
            HAlign::Left => 0.0,
            HAlign::Center => 0.5,
            HAlign::Right => 1.0,
        };
        let (fx, fy) = if matches!(valign, VAlign::Baseline) {
            let (sx, sy) = rot(layout.width, 0.0);
            (hfudge * sx, hfudge * sy)
        } else {
            let vfudge = match valign {
                VAlign::Bottom => 0.0,
                VAlign::Middle => 0.5,
                VAlign::Top => 1.0,
                VAlign::Baseline => 0.0,
            };
            (bx0 + hfudge * (bx1 - bx0), by0 + vfudge * (by1 - by0))
        };

        // Hit-test record, one shape per string (not per glyph): axis-aligned
        // text records its device bbox; angled text records the tight rotated
        // quad instead (the axis-aligned bbox of diagonal text overstates by
        // up to √2 and would capture clicks aimed at what's underneath).
        if self.recorder.is_some() {
            if (sin * cos).abs() < 1e-6 {
                self.record(RecordShape::Rect(Bounds {
                    x0: ax + em_px * (bx0 - fx),
                    y0: ay - em_px * (by1 - fy),
                    x1: ax + em_px * (bx1 - fx),
                    y1: ay - em_px * (by0 - fy),
                }));
            } else {
                let quad = [(ux0, uy0), (ux1, uy0), (ux1, uy1), (ux0, uy1)]
                    .into_iter()
                    .map(|(qx, qy)| {
                        let (rx, ry) = rot(qx, qy);
                        (ax + em_px * (rx - fx), ay - em_px * (ry - fy))
                    })
                    .collect();
                self.record(RecordShape::Quad(quad));
            }
        }

        let default_color = self.resolve(color);
        for g in &layout.glyphs {
            let outline = self.fonts.outline_char(g.font, g.ch);
            let Some(path) = &outline.path else { continue };
            // Map glyph outline (em, Y up): apply the run's text matrix,
            // place at the pen, rotate the whole string, then translate so
            // the fudge point sits at the device anchor (Y down). Composite
            // affine of R(angle) * tm with the device Y flip:
            let m = &g.tm;
            let tx = ax + em_px * ((g.x * cos - g.y * sin) - fx);
            let ty = ay - em_px * ((g.x * sin + g.y * cos) - fy);
            let ts = Transform::from_row(
                em_px * (cos * m.xx - sin * m.yx),
                -em_px * (sin * m.xx + cos * m.yx),
                em_px * (cos * m.xy - sin * m.yy),
                -em_px * (sin * m.xy + cos * m.yy),
                tx,
                ty,
            );
            // `transform` consumes the path, so clone the cached outline —
            // a plain buffer copy, far cheaper than re-outlining the glyph.
            let Some(tpath) = path.clone().transform(ts) else {
                continue;
            };
            let col = match g.color {
                Some(idx) => self.resolve(idx),
                None => default_color,
            };
            self.backend
                .fill_path(&tpath, &FillPaint::Solid(col), FillRule::Winding);
        }

        // Under/overline rules from \u / \o markup, as rectangles in the
        // rotated text frame.
        for r in &layout.rules {
            let (x0, x1) = (r.x0, r.x1);
            let (yb, yt) = (r.y, r.y + r.thickness);
            let corners = [(x0, yb), (x1, yb), (x1, yt), (x0, yt)];
            let mut pb = PathBuilder::new();
            for (i, (cx, cy)) in corners.iter().enumerate() {
                let px = ax + em_px * ((cx * cos - cy * sin) - fx);
                let py = ay - em_px * ((cx * sin + cy * cos) - fy);
                if i == 0 {
                    pb.move_to(px, py);
                } else {
                    pb.line_to(px, py);
                }
            }
            pb.close();
            let Some(path) = pb.finish() else { continue };
            let col = match r.color {
                Some(idx) => self.resolve(idx),
                None => default_color,
            };
            self.backend
                .fill_path(&path, &FillPaint::Solid(col), FillRule::Winding);
        }
    }
}

/// Polylines beyond this many device points get M4-decimated (solid style
/// only). Far above any normal plot; only data-dump-sized sets qualify.
const DENSE_POLYLINE_LIMIT: usize = 4096;

/// M4 aggregation (Jugel et al.): per device x-pixel column keep the first,
/// lowest, highest and last point, in index order. For a ~1px stroke this
/// reproduces the same column envelope as drawing every segment.
fn m4_decimate(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::new();
    let mut i = 0;
    while i < pts.len() {
        let col = pts[i].0.floor();
        let (mut min_i, mut max_i) = (i, i);
        let mut j = i;
        while j < pts.len() && pts[j].0.floor() == col {
            if pts[j].1 < pts[min_i].1 {
                min_i = j;
            }
            if pts[j].1 > pts[max_i].1 {
                max_i = j;
            }
            j += 1;
        }
        let last = j - 1;
        let (a, b) = (min_i.min(max_i), min_i.max(max_i));
        for idx in [i, a, b, last] {
            if out.last() != Some(&pts[idx]) {
                out.push(pts[idx]);
            }
        }
        i = j;
    }
    out
}

/// Dash pattern (in px) for a Grace line style index, or `None` for solid.
///
/// These are Grace's nine `dash_array` patterns (`patterns.h`); each value is a
/// multiple of the line width (as in Qt's `setDashPattern`), so we scale by the
/// device line width. Style 0 = none, 1 = solid (both `None` here; style 0 is
/// skipped by the caller).
/// Grace's nine line-style dash patterns (`patterns.h`), as on/off run
/// lengths in multiples of the line width, indexed by line style. Styles
/// 0 (none) and 1 (solid) have no pattern. Public so UIs can render
/// faithful line-style previews from the same source of truth.
pub const DASH_PATTERNS: [&[f32]; 9] = [
    &[],                                  // 0 none
    &[],                                  // 1 solid
    &[1.0, 3.0],                          // 2 dotted
    &[5.0, 3.0],                          // 3 dashed
    &[7.0, 3.0],                          // 4 long dash
    &[1.0, 3.0, 5.0, 3.0],                // 5 dot-dash
    &[1.0, 3.0, 7.0, 3.0],                // 6 dot-longdash
    &[1.0, 3.0, 5.0, 3.0, 1.0, 3.0],      // 7 dot-dash-dot
    &[5.0, 3.0, 1.0, 3.0, 5.0, 3.0],      // 8 dash-dot-dash
];

fn dash_pattern(linestyle: i32, width: f32) -> Option<Vec<f32>> {
    let u = width.max(1.0);
    let pat = usize::try_from(linestyle)
        .ok()
        .and_then(|i| DASH_PATTERNS.get(i))
        .filter(|p| !p.is_empty())?;
    Some(pat.iter().map(|d| d * u).collect())
}
