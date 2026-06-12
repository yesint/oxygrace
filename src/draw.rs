//! The high-level draw layer: turns a [`crate::model::Project`] into pixels by
//! orchestrating the plot ([`plot`]), axes ([`axes`]) and datasets ([`sets`]).

pub mod axes;
pub mod objects;
pub mod decor;
pub mod pie;
pub mod plot;
pub mod sets;

pub use plot::{draw_project, draw_project_pixmap, draw_project_svg};
