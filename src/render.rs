//! Rendering primitives: the [`canvas`] with its raster (tiny-skia) and
//! vector ([`svg`]) backends, the coordinate [`transform`]s between world,
//! view and device space, and the hit-test [`record`]ing side-channel.

pub mod canvas;
pub mod record;
pub mod svg;
pub mod transform;

pub use canvas::{Canvas, HAlign, VAlign, VPoint};
pub use record::{Bounds, ElementId, OverlayShape, RenderInfo};
pub use transform::{PageTransform, WorldTransform};
