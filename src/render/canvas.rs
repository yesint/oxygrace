//! The drawing canvas: a [`tiny_skia::Pixmap`] plus the device primitives the
//! draw layer calls (polylines, filled polygons, text), all taking **view**
//! coordinates and color *indices*, which are resolved against the project's
//! color map here.

use tiny_skia::{
    FillRule, FilterQuality, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pattern, Pixmap,
    SpreadMode, Stroke, StrokeDash, Transform,
};

use crate::color::{self, Rgba};
use crate::font::FontSet;
use crate::model::Project;
use crate::patterns::PATTERN_BITS;
use crate::render::transform::PageTransform;
use crate::text;

/// Build a 16x16 RGBA tile for a Grace fill pattern in the given color.
/// Set bits get the opaque color; unset bits are transparent.
fn pattern_tile(pattern: i32, color: Rgba) -> Option<Pixmap> {
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
            if (byte >> (col % 8)) & 1 == 1 {
                // Premultiplied opaque color.
                px[row * 16 + col] =
                    tiny_skia::PremultipliedColorU8::from_rgba(color.r, color.g, color.b, 255)
                        .unwrap();
            }
        }
    }
    Some(tile)
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

/// Wraps the output pixmap and the page transform; resolves colors via the
/// borrowed [`Project`] and outlines glyphs via the borrowed [`FontSet`].
pub struct Canvas<'a> {
    pixmap: Pixmap,
    page: PageTransform,
    project: &'a Project,
    fonts: &'a FontSet,
    /// Active clip region (device-space mask), or `None` for unclipped
    /// drawing. Mirrors Grace's `setclipping` + per-graph `viewport` clip in
    /// `draw.cpp` (`clip_line`/`clip_polygon`): data elements are clipped to
    /// the graph viewport, decorations are not.
    clip: Option<Mask>,
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
            clip: None,
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
        let mut mask = Mask::new(self.pixmap.width(), self.pixmap.height())
            .expect("non-zero page dimensions");
        if let Some(rect) = tiny_skia::Rect::from_ltrb(x0, y0, x1, y1) {
            let path = PathBuilder::from_rect(rect);
            mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        }
        self.clip = Some(mask);
    }

    /// Disable clipping (Grace `setclipping(FALSE)`).
    pub fn clear_clip(&mut self) {
        self.clip = None;
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
            .stroke_path(&path, &paint, &stroke, Transform::identity(), self.clip.as_ref());
    }

    /// Fill a closed polygon (view coords) with a color index and fill pattern.
    /// Pattern 0 = no fill, 1 = solid, 2..=31 = a tiled hatch in `color`.
    pub fn fill_polygon(&mut self, pts: &[VPoint], color: i32, pattern: i32) {
        self.fill_polygon_rule(pts, color, pattern, 0);
    }

    /// Like [`Canvas::fill_polygon`] with an explicit fill rule
    /// (0 = winding, 1 = even-odd; Grace `setfillrule` in `drawsetfill`).
    pub fn fill_polygon_rule(&mut self, pts: &[VPoint], color: i32, pattern: i32, rule: i32) {
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
        let rule = if rule == 1 {
            FillRule::EvenOdd
        } else {
            FillRule::Winding
        };
        self.fill_path_rule(&path, color, pattern, rule);
    }

    /// Fill a path with a color index and fill pattern (shared by all fills).
    fn fill_path_pen(&mut self, path: &Path, color: i32, pattern: i32) {
        self.fill_path_rule(path, color, pattern, FillRule::Winding);
    }

    /// Fill a path with a color index, fill pattern and explicit fill rule.
    fn fill_path_rule(&mut self, path: &Path, color: i32, pattern: i32, rule: FillRule) {
        if pattern == 0 {
            return;
        }
        let rgba = color::resolve(self.project, color);
        let mut paint = Paint {
            anti_alias: true,
            ..Default::default()
        };
        if pattern == 1 {
            paint.set_color(rgba.to_skia());
            self.pixmap
                .fill_path(path, &paint, rule, Transform::identity(), self.clip.as_ref());
            return;
        }
        // Hatched pattern: tile a 16x16 stencil in the foreground color.
        let Some(tile) = pattern_tile(pattern, rgba) else {
            // Unknown pattern -> fall back to solid.
            paint.set_color(rgba.to_skia());
            self.pixmap
                .fill_path(path, &paint, rule, Transform::identity(), self.clip.as_ref());
            return;
        };
        paint.shader = Pattern::new(
            tile.as_ref(),
            SpreadMode::Repeat,
            FilterQuality::Nearest,
            1.0,
            Transform::identity(),
        );
        self.pixmap
            .fill_path(path, &paint, rule, Transform::identity(), self.clip.as_ref());
    }

    /// Width of a marked-up string in view units, at the given char size.
    pub fn text_width_view(&self, s: &str, charsize: f64, font: i32) -> f64 {
        let em = text::measure(self.fonts, s, font) as f64;
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
        match text::bbox(self.fonts, s, font) {
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
            .stroke_path(&path, &paint, &stroke, Transform::identity(), self.clip.as_ref());
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
        let theta = angle.to_radians() as f32;
        let (sin, cos) = (theta.sin(), theta.cos());
        let rot = |x: f32, y: f32| (x * cos - y * sin, x * sin + y * cos);

        // Axis-aligned bounding box of the *rotated* glyph outlines, in em
        // units (Y up) — Grace's `bbox_ll`/`bbox_ur` accumulated in WriteString.
        let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut any = false;
        for g in &layout.glyphs {
            if let Some((gx0, gy0, gx1, gy1)) = self.fonts.glyph_bbox(g.font, g.ch) {
                for &(cx, cy) in &[(gx0, gy0), (gx1, gy0), (gx1, gy1), (gx0, gy1)] {
                    let (rx, ry) = rot(g.x + cx * g.scale, g.y + cy * g.scale);
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

        let default_color = color::resolve(self.project, color).to_skia();
        for g in &layout.glyphs {
            let outline = self.fonts.outline_char(g.font, g.ch);
            let Some(path) = outline.path else { continue };
            let s = g.scale;
            // Map glyph outline (em, Y up): scale, place at pen, rotate, then
            // translate so the fudge point sits at the device anchor (Y down).
            let tx = ax + em_px * ((g.x * cos - g.y * sin) - fx);
            let ty = ay - em_px * ((g.x * sin + g.y * cos) - fy);
            let ts = Transform::from_row(
                em_px * s * cos,
                -em_px * s * sin,
                -em_px * s * sin,
                -em_px * s * cos,
                tx,
                ty,
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
/// These are Grace's nine `dash_array` patterns (`patterns.h`); each value is a
/// multiple of the line width (as in Qt's `setDashPattern`), so we scale by the
/// device line width. Style 0 = none, 1 = solid (both `None` here; style 0 is
/// skipped by the caller).
fn dash_pattern(linestyle: i32, width: f32) -> Option<Vec<f32>> {
    let u = width.max(1.0);
    let pat: &[f32] = match linestyle {
        2 => &[1.0, 3.0],                // dotted
        3 => &[5.0, 3.0],                // dashed
        4 => &[7.0, 3.0],                // long dash
        5 => &[1.0, 3.0, 5.0, 3.0],      // dot-dash
        6 => &[1.0, 3.0, 7.0, 3.0],      // dot-longdash
        7 => &[1.0, 3.0, 5.0, 3.0, 1.0, 3.0], // dot-dash-dot
        8 => &[5.0, 3.0, 1.0, 3.0, 5.0, 3.0], // dash-dot-dash
        _ => return None,                // 0/1 -> solid
    };
    Some(pat.iter().map(|d| d * u).collect())
}
