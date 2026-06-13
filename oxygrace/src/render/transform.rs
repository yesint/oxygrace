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

    /// Device pixel to view point (inverse of [`PageTransform::view_to_device`]).
    pub fn device_to_view(&self, px: f32, py: f32) -> (f64, f64) {
        (px as f64 / self.side, (self.height - py as f64) / self.side)
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

/// Inverse of [`scale_fwd`] (scaled coordinate -> world value).
fn scale_inv(scale: ScaleType, s: f64) -> Option<f64> {
    match scale {
        ScaleType::Normal => Some(s),
        ScaleType::Logarithmic => Some(10f64.powf(s)),
        ScaleType::Reciprocal => {
            if s != 0.0 {
                Some(1.0 / s)
            } else {
                None
            }
        }
        // Inverse of ln(v / (1 - v)) — the logistic function.
        ScaleType::Logit => Some(1.0 / (1.0 + (-s).exp())),
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
    /// Fixed graphs: both axes share one world->view rate, anchored at the
    /// viewport/world midpoints (definewindow_local GRAPH_FIXED) — the
    /// viewport itself is untouched; the frame stays at the file viewport
    /// while data and axes land in a world-aspect sub-area.
    fixed: bool,
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
            fixed: graph.graph_type == crate::model::GraphType::Fixed,
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

    /// Shared world->view rate of a Fixed graph: the smaller of the two
    /// axis rates (definewindow_local GRAPH_FIXED `xv_rc`).
    fn fixed_rate(&self) -> f64 {
        let rx = (self.vx1 - self.vx0) / (self.sx1 - self.sx0);
        let ry = (self.vy1 - self.vy0) / (self.sy1 - self.sy0);
        rx.min(ry)
    }

    /// Map a world X value to its view X coordinate (0 if out of domain,
    /// per Grace `xy_xconv_general`).
    pub fn x_to_view(&self, wx: f64) -> f64 {
        let Some(sx) = scale_fwd(self.xscale, wx) else {
            return 0.0;
        };
        if self.fixed {
            let med = (self.vx0 + self.vx1) / 2.0;
            let wmed = (self.sx0 + self.sx1) / 2.0;
            let rc = if self.xinvert { -self.fixed_rate() } else { self.fixed_rate() };
            return med + rc * (sx - wmed);
        }
        let mut fx = (sx - self.sx0) / (self.sx1 - self.sx0);
        if self.xinvert {
            fx = 1.0 - fx;
        }
        self.vx0 + fx * (self.vx1 - self.vx0)
    }

    /// View X coordinate back to the world X value (inverse of
    /// [`WorldTransform::x_to_view`]; `None` when out of the scale's domain).
    pub fn view_to_x(&self, vx: f64) -> Option<f64> {
        let sx = if self.fixed {
            let med = (self.vx0 + self.vx1) / 2.0;
            let wmed = (self.sx0 + self.sx1) / 2.0;
            let rc = if self.xinvert { -self.fixed_rate() } else { self.fixed_rate() };
            wmed + (vx - med) / rc
        } else {
            let mut fx = (vx - self.vx0) / (self.vx1 - self.vx0);
            if self.xinvert {
                fx = 1.0 - fx;
            }
            self.sx0 + fx * (self.sx1 - self.sx0)
        };
        scale_inv(self.xscale, sx)
    }

    /// View Y coordinate back to the world Y value (inverse of
    /// [`WorldTransform::y_to_view`]).
    pub fn view_to_y(&self, vy: f64) -> Option<f64> {
        let sy = if self.fixed {
            let med = (self.vy0 + self.vy1) / 2.0;
            let wmed = (self.sy0 + self.sy1) / 2.0;
            let rc = if self.yinvert { -self.fixed_rate() } else { self.fixed_rate() };
            wmed + (vy - med) / rc
        } else {
            let mut fy = (vy - self.vy0) / (self.vy1 - self.vy0);
            if self.yinvert {
                fy = 1.0 - fy;
            }
            self.sy0 + fy * (self.sy1 - self.sy0)
        };
        scale_inv(self.yscale, sy)
    }

    /// View point back to world coordinates (inverse of
    /// [`WorldTransform::world_to_view`]); polar graphs return (phi, rho).
    pub fn view_to_world(&self, vx: f64, vy: f64) -> Option<(f64, f64)> {
        if self.polar {
            let (cx, cy, rc) = self.polar_params();
            let (dx, dy) = (vx - cx, vy - cy);
            let rho = (dx * dx + dy * dy).sqrt() / rc;
            let sign = if self.xinvert { -1.0 } else { 1.0 };
            return Some((sign * dy.atan2(dx), rho));
        }
        Some((self.view_to_x(vx)?, self.view_to_y(vy)?))
    }

    /// Pan the world window by a view-space delta (a drag in view units),
    /// preserving the window width. The shift is computed in *scaled* space
    /// so logarithmic / reciprocal / logit axes pan uniformly on screen.
    /// Returns the new `(xmin, xmax, ymin, ymax)`; content follows the drag
    /// (a positive `dvx` moves the data right). Polar graphs are left
    /// unchanged.
    pub fn pan_world(&self, dvx: f64, dvy: f64) -> (f64, f64, f64, f64) {
        if self.polar {
            return (self.wxmin, self.wxmax, self.wymin, self.wymax);
        }
        let shift = |s0: f64, s1: f64, dv: f64, v0: f64, v1: f64, invert: bool| {
            let mut f = dv / (v1 - v0);
            if invert {
                f = -f;
            }
            let w = s1 - s0;
            (s0 - f * w, s1 - f * w)
        };
        let (nx0, nx1) = shift(self.sx0, self.sx1, dvx, self.vx0, self.vx1, self.xinvert);
        let (ny0, ny1) = shift(self.sy0, self.sy1, dvy, self.vy0, self.vy1, self.yinvert);
        (
            scale_inv(self.xscale, nx0).unwrap_or(self.wxmin),
            scale_inv(self.xscale, nx1).unwrap_or(self.wxmax),
            scale_inv(self.yscale, ny0).unwrap_or(self.wymin),
            scale_inv(self.yscale, ny1).unwrap_or(self.wymax),
        )
    }

    /// Map a world Y value to its view Y coordinate (0 if out of domain,
    /// per Grace `xy_yconv_general`).
    pub fn y_to_view(&self, wy: f64) -> f64 {
        let Some(sy) = scale_fwd(self.yscale, wy) else {
            return 0.0;
        };
        if self.fixed {
            let med = (self.vy0 + self.vy1) / 2.0;
            let wmed = (self.sy0 + self.sy1) / 2.0;
            let rc = if self.yinvert { -self.fixed_rate() } else { self.fixed_rate() };
            return med + rc * (sy - wmed);
        }
        let mut fy = (sy - self.sy0) / (self.sy1 - self.sy0);
        if self.yinvert {
            fy = 1.0 - fy;
        }
        self.vy0 + fy * (self.vy1 - self.vy0)
    }
}
