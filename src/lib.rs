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
pub mod model;
pub mod parse;
pub mod patterns;
pub mod render;
pub mod text;

pub use model::Project;

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

/// Render a project to an SVG document using the bundled fonts. Text is
/// emitted as glyph outline paths, so the result displays identically
/// everywhere and matches the PNG rendering.
pub fn render_svg(project: &Project) -> String {
    let fonts = font::FontSet::load();
    draw::draw_project_svg(project, &fonts)
}
