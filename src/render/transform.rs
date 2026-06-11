//! Coordinate and size transforms.
//!
//! Grace uses three spaces:
//! * **world** — data coordinates (the axes' units),
//! * **view** — normalized page coordinates; both axes are scaled by the same
//!   factor `side = min(page_w, page_h)`, so view space is isotropic and the
//!   origin is the bottom-left of the page,
//! * **device** — pixel coordinates of the output image, origin top-left, Y
//!   pointing down.
//!
//! The magic constants (`MAGIC_LINEW_SCALE`, `MAGIC_FONT_SCALE`) and the
//! `side`-based scaling are taken from QtGrace6 (`src/globals.h`,
//! `src/t1fonts.h`, `src/svgdrv.cpp`).

use crate::model::{Graph, ScaleType, World};

/// Grace's line-width-to-view scale (`MAGIC_LINEW_SCALE`).
pub const MAGIC_LINEW_SCALE: f64 = 0.0015;
/// Grace's char-size-to-view font scale (`MAGIC_FONT_SCALE`).
pub const MAGIC_FONT_SCALE: f64 = 0.028;

/// Maps view coordinates to device pixels for a given page.
#[derive(Debug, Clone, Copy)]
pub struct PageTransform {
    pub width: f64,
    pub height: f64,
    /// `min(width, height)` — the isotropic view→device scale factor.
    pub side: f64,
}

impl PageTransform {
    pub fn new(width: u32, height: u32) -> Self {
        let width = width as f64;
        let height = height as f64;
        PageTransform {
            width,
            height,
            side: width.min(height),
        }
    }

    /// View point to device pixel (origin top-left, Y down). Isotropic: both
    /// axes scale by [`PageTransform::side`].
    pub fn view_to_device(&self, vx: f64, vy: f64) -> (f32, f32) {
        let px = vx * self.side;
        let py = self.height - vy * self.side;
        (px as f32, py as f32)
    }

    /// Convert a Grace line width to device pixels.
    pub fn linewidth_px(&self, linew: f64) -> f32 {
        (linew * MAGIC_LINEW_SCALE * self.side) as f32
    }

    /// Convert a Grace character size to an em height in device pixels.
    pub fn fontsize_px(&self, charsize: f64) -> f32 {
        (charsize * MAGIC_FONT_SCALE * self.side) as f32
    }

    /// Convert a length given in view units to device pixels.
    pub fn view_len_px(&self, len: f64) -> f32 {
        (len * self.side) as f32
    }
}

/// Apply a single-axis scale transform (world value -> monotonic coordinate).
///
/// Logarithmic/reciprocal/logit values that are out of their domain return
/// `None` so the caller can skip the point.
fn scale_fwd(scale: ScaleType, v: f64) -> Option<f64> {
    match scale {
        ScaleType::Normal => Some(v),
        ScaleType::Logarithmic => {
            if v > 0.0 {
                Some(v.log10())
            } else {
                None
            }
        }
        ScaleType::Reciprocal => {
            if v != 0.0 {
                Some(1.0 / v)
            } else {
                None
            }
        }
        ScaleType::Logit => {
            if v > 0.0 && v < 1.0 {
                Some((v / (1.0 - v)).ln())
            } else {
                None
            }
        }
    }
}

/// Maps a graph's world coordinates to view coordinates.
#[derive(Debug, Clone, Copy)]
pub struct WorldTransform {
    xscale: ScaleType,
    yscale: ScaleType,
    xinvert: bool,
    yinvert: bool,
    /// Polar graphs: world x is the angle phi, world y the radius rho
    /// (draw.cpp `COORDINATES_POLAR`).
    polar: bool,
    /// World rho-max and the world window (for the polar point test).
    wymin: f64,
    wymax: f64,
    wxmin: f64,
    wxmax: f64,
    // Pre-scaled world window bounds.
    sx0: f64,
    sx1: f64,
    sy0: f64,
    sy1: f64,
    // Viewport bounds.
    vx0: f64,
    vx1: f64,
    vy0: f64,
    vy1: f64,
}

impl WorldTransform {
    /// Build the transform from a graph's world window, viewport and scales.
    pub fn new(graph: &Graph) -> Self {
        let World {
            xmin,
            xmax,
            ymin,
            ymax,
        } = graph.world;
        let v = graph.view;
        // Pre-scale the window bounds; fall back to raw values if out of domain.
        let sx0 = scale_fwd(graph.xscale, xmin).unwrap_or(xmin);
        let sx1 = scale_fwd(graph.xscale, xmax).unwrap_or(xmax);
        let sy0 = scale_fwd(graph.yscale, ymin).unwrap_or(ymin);
        let sy1 = scale_fwd(graph.yscale, ymax).unwrap_or(ymax);
        WorldTransform {
            xscale: graph.xscale,
            yscale: graph.yscale,
            xinvert: graph.xinvert,
            yinvert: graph.yinvert,
            polar: graph.graph_type == crate::model::GraphType::Polar,
            wymin: ymin,
            wymax: ymax,
            wxmin: xmin,
            wxmax: xmax,
            sx0,
            sx1,
            sy0,
            sy1,
            vx0: v.xmin,
            vx1: v.xmax,
            vy0: v.ymin,
            vy1: v.ymax,
        }
    }

    /// World point to view point.
    ///
    /// A value outside its scale's domain (e.g. y <= 0 on a log axis) maps to
    /// view coordinate **0** for that axis, exactly like Grace's
    /// `xy_xconv_general` / `xy_yconv_general` (`draw.cpp`); the viewport clip
    /// then trims whatever geometry reaches toward the page corner. Grace
    /// never skips such points.
    pub fn world_to_view(&self, wx: f64, wy: f64) -> (f64, f64) {
        if self.polar {
            // Polar graphs: world (phi, rho) maps to the viewport center plus
            // rho-scaled (cos, sin); phi flips with xinvert (definewindow
            // GRAPH_POLAR: xv_rc = +-1, yv_rc = min(w,h)/2 / rho_max).
            let (cx, cy, rc) = self.polar_params();
            let sign = if self.xinvert { -1.0 } else { 1.0 };
            let phi = sign * wx;
            return (cx + rc * wy * phi.cos(), cy + rc * wy * phi.sin());
        }
        (self.x_to_view(wx), self.y_to_view(wy))
    }

    /// Polar center and rho->view rate.
    pub fn polar_params(&self) -> (f64, f64, f64) {
        let cx = (self.vx0 + self.vx1) / 2.0;
        let cy = (self.vy0 + self.vy1) / 2.0;
        let rc = 0.5 * (self.vx1 - self.vx0).min(self.vy1 - self.vy0) / self.wymax;
        (cx, cy, rc)
    }

    /// Grace `is_validWPoint`: inside the world window, except polar graphs
    /// only restrict the radius to 0..rho_max (draw.cpp).
    pub fn valid_wpoint(&self, wx: f64, wy: f64) -> bool {
        if self.polar {
            return wy >= 0.0 && wy <= self.wymax;
        }
        wx >= self.wxmin.min(self.wxmax)
            && wx <= self.wxmin.max(self.wxmax)
            && wy >= self.wymin.min(self.wymax)
            && wy <= self.wymin.max(self.wymax)
    }

    /// Map a world X value to its view X coordinate (0 if out of domain,
    /// per Grace `xy_xconv_general`).
    pub fn x_to_view(&self, wx: f64) -> f64 {
        let Some(sx) = scale_fwd(self.xscale, wx) else {
            return 0.0;
        };
        let mut fx = (sx - self.sx0) / (self.sx1 - self.sx0);
        if self.xinvert {
            fx = 1.0 - fx;
        }
        self.vx0 + fx * (self.vx1 - self.vx0)
    }

    /// Map a world Y value to its view Y coordinate (0 if out of domain,
    /// per Grace `xy_yconv_general`).
    pub fn y_to_view(&self, wy: f64) -> f64 {
        let Some(sy) = scale_fwd(self.yscale, wy) else {
            return 0.0;
        };
        let mut fy = (sy - self.sy0) / (self.sy1 - self.sy0);
        if self.yinvert {
            fy = 1.0 - fy;
        }
        self.vy0 + fy * (self.vy1 - self.vy0)
    }
}
