//! Parsing of Grace `.agr`/`.xvg` files: the [`grammar`] (PEG command
//! language), the [`reader`] (line loop + model application) and numeric
//! [`data`] rows.

pub mod data;
pub mod grammar;
pub mod reader;

pub use reader::parse_project;
