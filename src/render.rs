//! Rendering primitives: the [`canvas`] (tiny-skia device) and the coordinate
//! [`transform`]s between world, view and device space.

pub mod canvas;
pub mod transform;

pub use canvas::{Canvas, HAlign, VAlign, VPoint};
pub use transform::{PageTransform, WorldTransform};
