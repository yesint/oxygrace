//! Oxygrace: a pure-Rust headless interpreter and renderer for Grace
//! (`.agr` / `.xvg`) plot files.
//!
//! The pipeline is: parse a file into a [`model::Project`] ([`load`] /
//! [`load_str`]), then rasterize it to a PNG with [`render_png`].
//!
//! ```no_run
//! let project = oxygrace::load("plot.agr").unwrap();
//! let png = oxygrace::render_png(&project);
//! std::fs::write("plot.png", png).unwrap();
//! ```

use std::path::Path;

pub mod color;
pub mod dates;
pub mod draw;
pub mod font;
pub mod import;
pub mod model;
pub mod parse;
pub mod patterns;
pub mod render;
pub mod text;
pub mod write;

pub use font::FontSet;
pub use model::Project;
pub use render::{Bounds, ElementId, OverlayShape, RenderInfo};
// Re-exported so the pixmap type in [`RenderResult`] is nameable without a
// separate (potentially version-skewed) tiny-skia dependency.
pub use tiny_skia;

/// Parse a `.agr`/`.xvg` file from disk into a [`Project`].
///
/// The file is decoded as UTF-8 when valid; otherwise it is decoded as
/// Latin-1 (every byte maps to the same code point), which is what
/// Grace-era files use for characters like the degree sign.
pub fn load<P: AsRef<Path>>(path: P) -> std::io::Result<Project> {
    let bytes = std::fs::read(path)?;
    let content = match std::str::from_utf8(&bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    };
    Ok(load_str(&content))
}

/// Parse `.agr`/`.xvg` text into a [`Project`].
pub fn load_str(content: &str) -> Project {
    parse::parse_project(content)
}

/// Render a project to PNG bytes using the bundled fonts.
pub fn render_png(project: &Project) -> Vec<u8> {
    let fonts = font::FontSet::load();
    draw::draw_project(project, &fonts)
}

/// A raster rendering plus the element geometry recorded while drawing it.
pub struct RenderResult {
    /// The page as a premultiplied-RGBA pixmap (white background, opaque).
    pub pixmap: tiny_skia::Pixmap,
    /// Recorded element geometry: `info.hit_test(x, y, tol)` answers "what
    /// is at device pixel (x, y)?", `info.bounds(id)` gives selection boxes.
    pub info: RenderInfo,
}

/// Render a project to a raw pixmap with hit-test geometry. Takes the
/// [`FontSet`] by reference so long-lived callers (a GUI) load fonts once.
pub fn render_pixmap(project: &Project, fonts: &FontSet) -> RenderResult {
    let (pixmap, info) = draw::draw_project_pixmap(project, fonts);
    RenderResult { pixmap, info }
}

/// Render a project to an SVG document using the bundled fonts. Text is
/// emitted as glyph outline paths, so the result displays identically
/// everywhere and matches the PNG rendering.
pub fn render_svg(project: &Project) -> String {
    let fonts = font::FontSet::load();
    draw::draw_project_svg(project, &fonts)
}

pub use write::{save, save_str};
