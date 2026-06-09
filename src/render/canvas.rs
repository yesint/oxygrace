//! The drawing canvas: a [`tiny_skia::Pixmap`] plus the device primitives the
//! draw layer calls (polylines, filled polygons, text), all taking **view**
//! coordinates and color *indices*, which are resolved against the project's
//! color map here.

use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

use crate::color::{self};
use crate::font::FontSet;
use crate::model::Project;
use crate::render::transform::PageTransform;
use crate::text;

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

/// Wraps the output pixmap and the page transform; resolves colors via the
/// borrowed [`Project`] and outlines glyphs via the borrowed [`FontSet`].
pub struct Canvas<'a> {
    pixmap: Pixmap,
    page: PageTransform,
    project: &'a Project,
    fonts: &'a FontSet,
}

impl<'a> Canvas<'a> {
    /// Create a white page sized from the project.
    pub fn new(project: &'a Project, fonts: &'a FontSet) -> Self {
        let mut pixmap = Pixmap::new(project.page_width, project.page_height)
            .expect("non-zero page dimensions");
        pixmap.fill(tiny_skia::Color::WHITE);
        Canvas {
            pixmap,
            page: PageTransform::new(project.page_width, project.page_height),
            project,
            fonts,
        }
    }

    /// Access the page transform (for size conversions in the draw layer).
    pub fn page(&self) -> &PageTransform {
        &self.page
    }

    /// Fill the whole page with a color index.
    pub fn fill_page(&mut self, color: i32) {
        self.pixmap.fill(color::resolve(self.project, color).to_skia());
    }

    /// Encode the pixmap as PNG bytes.
    pub fn to_png(&self) -> Vec<u8> {
        self.pixmap.encode_png().expect("PNG encoding")
    }

    /// Build a stroke for a line style index and width (in px).
    fn stroke(&self, linestyle: i32, width_px: f32) -> Stroke {
        let mut stroke = Stroke {
            width: width_px.max(0.1),
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            ..Stroke::default()
        };
        if let Some(dash) = dash_pattern(linestyle, width_px) {
            stroke.dash = StrokeDash::new(dash, 0.0);
        }
        stroke
    }

    /// Stroke a polyline given in view coordinates.
    pub fn draw_polyline(&mut self, pts: &[VPoint], color: i32, linewidth: f64, linestyle: i32) {
        if pts.len() < 2 || linestyle == 0 {
            return;
        }
        let mut pb = PathBuilder::new();
        for (i, p) in pts.iter().enumerate() {
            let (x, y) = self.page.view_to_device(p.x, p.y);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        let Some(path) = pb.finish() else { return };
        let mut paint = Paint::default();
        paint.set_color(color::resolve(self.project, color).to_skia());
        paint.anti_alias = true;
        let stroke = self.stroke(linestyle, self.page.linewidth_px(linewidth));
        self.pixmap
            .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    /// Fill a closed polygon given in view coordinates.
    pub fn fill_polygon(&mut self, pts: &[VPoint], color: i32) {
        if pts.len() < 3 {
            return;
        }
        let mut pb = PathBuilder::new();
        for (i, p) in pts.iter().enumerate() {
            let (x, y) = self.page.view_to_device(p.x, p.y);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        let Some(path) = pb.finish() else { return };
        let mut paint = Paint::default();
        paint.set_color(color::resolve(self.project, color).to_skia());
        paint.anti_alias = true;
        self.pixmap
            .fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }

    /// Fill a circle (center + radius in view units) with a color index.
    pub fn fill_circle(&mut self, center: VPoint, radius_view: f64, color: i32) {
        let (cx, cy) = self.page.view_to_device(center.x, center.y);
        let r = self.page.view_len_px(radius_view);
        if r <= 0.0 {
            return;
        }
        let Some(path) = PathBuilder::from_circle(cx, cy, r) else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color(color::resolve(self.project, color).to_skia());
        paint.anti_alias = true;
        self.pixmap
            .fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
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
        let mut paint = Paint::default();
        paint.set_color(color::resolve(self.project, color).to_skia());
        paint.anti_alias = true;
        let stroke = self.stroke(ls, self.page.linewidth_px(lw));
        self.pixmap
            .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
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
        let layout = text::layout(self.fonts, s, base_font);
        if layout.glyphs.is_empty() {
            return;
        }
        let em_px = self.page.fontsize_px(charsize);
        let (ax, ay) = self.page.view_to_device(anchor.x, anchor.y);

        // Alignment offsets in text space (y up).
        let halign_off = match halign {
            HAlign::Left => 0.0,
            HAlign::Center => layout.width * em_px / 2.0,
            HAlign::Right => layout.width * em_px,
        };
        let ascent = self.fonts.ascent(base_font) * em_px;
        let descent = self.fonts.descent(base_font) * em_px; // negative
        let valign_off = match valign {
            VAlign::Baseline => 0.0,
            VAlign::Bottom => descent,
            VAlign::Middle => (ascent + descent) / 2.0,
            VAlign::Top => ascent,
        };

        let theta = angle.to_radians() as f32;
        let (sin, cos) = (theta.sin(), theta.cos());
        let default_color = color::resolve(self.project, color).to_skia();

        for g in &layout.glyphs {
            let outline = self.fonts.outline_char(g.font, g.ch);
            let Some(path) = outline.path else { continue };
            let sx = em_px * g.scale;
            // Per-glyph text-space origin (before rotation), y up.
            let cx = em_px * g.x - halign_off;
            let cy = em_px * g.y - valign_off;
            // Compose: glyph-em (y up) -> rotate -> device (y down) -> anchor.
            let ts = Transform::from_row(
                cos * sx,
                -sin * sx,
                -sin * sx,
                -cos * sx,
                ax + cos * cx - sin * cy,
                ay - sin * cx - cos * cy,
            );
            let Some(tpath) = path.transform(ts) else {
                continue;
            };
            let mut paint = Paint::default();
            let col = match g.color {
                Some(idx) => color::resolve(self.project, idx).to_skia(),
                None => default_color,
            };
            paint.set_color(col);
            paint.anti_alias = true;
            self.pixmap
                .fill_path(&tpath, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}

/// Dash pattern (in px) for a Grace line style index, or `None` for solid.
///
/// Approximates Grace's nine line styles (`patterns.h`); style 1 is solid.
/// Lengths scale with line width so dashes stay visible on thick lines.
fn dash_pattern(linestyle: i32, width: f32) -> Option<Vec<f32>> {
    let u = width.max(1.0);
    let pat: &[f32] = match linestyle {
        2 => &[4.0, 2.0],            // dotted-ish
        3 => &[8.0, 4.0],            // dashed
        4 => &[1.0, 3.0],            // dotted
        5 => &[8.0, 4.0, 1.0, 4.0],  // dash-dot
        6 => &[12.0, 4.0],           // long dash
        7 => &[12.0, 4.0, 1.0, 4.0], // long dash-dot
        8 => &[1.0, 2.0, 8.0, 2.0],  // dot-dash
        _ => return None,            // 0/1 -> solid (0 handled by caller)
    };
    Some(pat.iter().map(|d| d * u).collect())
}
