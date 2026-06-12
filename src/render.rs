//! Rendering primitives: the [`canvas`] with its raster (tiny-skia) and
//! vector ([`svg`]) backends, and the coordinate [`transform`]s between
//! world, view and device space.

pub mod canvas;
pub mod svg;
pub mod transform;

pub use canvas::{Canvas, HAlign, VAlign, VPoint};
pub use transform::{PageTransform, WorldTransform};
