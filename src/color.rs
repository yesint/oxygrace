//! Color handling: Grace's default 16-entry colormap plus per-project
//! overrides from `@map color`.
//!
//! RGB values are taken verbatim from QtGrace6 `src/draw.cpp` (`cmap_init`).

use crate::model::Project;

/// Grace's built-in 16 colors, indexed 0..=15, as `(r, g, b)`.
pub const DEFAULT_COLORMAP: [(u8, u8, u8); 16] = [
    (255, 255, 255), // 0  white (background)
    (0, 0, 0),       // 1  black (foreground)
    (255, 0, 0),     // 2  red
    (0, 255, 0),     // 3  green
    (0, 0, 255),     // 4  blue
    (255, 255, 0),   // 5  yellow
    (188, 143, 143), // 6  brown
    (220, 220, 220), // 7  grey
    (148, 0, 211),   // 8  violet
    (0, 255, 255),   // 9  cyan
    (255, 0, 255),   // 10 magenta
    (255, 165, 0),   // 11 orange
    (114, 33, 188),  // 12 indigo
    (103, 7, 72),    // 13 maroon
    (64, 224, 208),  // 14 turquoise
    (0, 139, 0),     // 15 green4 / forest green
];

/// An opaque RGBA color (alpha currently always 255).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 255 }
    }

    /// Convert to a `tiny_skia::Color`.
    pub fn to_skia(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

/// Resolve a color index to RGBA, honoring `@map color` overrides and falling
/// back to the default colormap. Out-of-range indices resolve to black.
pub fn resolve(project: &Project, index: i32) -> Rgba {
    if let Some(&(_, (r, g, b))) = project
        .color_overrides
        .iter()
        .find(|&&(i, _)| i == index)
    {
        return Rgba::rgb(r, g, b);
    }
    if (0..DEFAULT_COLORMAP.len() as i32).contains(&index) {
        let (r, g, b) = DEFAULT_COLORMAP[index as usize];
        Rgba::rgb(r, g, b)
    } else {
        Rgba::rgb(0, 0, 0)
    }
}
