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
pub mod draw;
pub mod font;
pub mod model;
pub mod parse;
pub mod render;
pub mod text;

pub use model::Project;

/// Parse a `.agr`/`.xvg` file from disk into a [`Project`].
///
/// The file is decoded as UTF-8 leniently: invalid byte sequences (e.g. legacy
/// Latin-1 files) are replaced rather than rejected, so older Grace files still
/// load.
pub fn load<P: AsRef<Path>>(path: P) -> std::io::Result<Project> {
    let bytes = std::fs::read(path)?;
    let content = String::from_utf8_lossy(&bytes);
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
