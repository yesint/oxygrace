//! Grace's global default drawing properties (`@default ...`).
//!
//! These seed every newly created set/object. The initial values match
//! Grace's built-in defaults.

/// Default pen / text properties applied to new objects.
#[derive(Debug, Clone, Copy)]
pub struct Defaults {
    /// Default color index (1 = black).
    pub color: i32,
    /// Default fill pattern index (1 = solid).
    pub pattern: i32,
    /// Default line style index (1 = solid).
    pub linestyle: i32,
    /// Default line width (Grace units).
    pub linewidth: f64,
    /// Default character size multiplier.
    pub charsize: f64,
    /// Default font slot (0 = Times-Roman).
    pub font: i32,
    /// Default symbol size multiplier.
    pub symsize: f64,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            color: 1,
            pattern: 1,
            linestyle: 1,
            linewidth: 1.0,
            charsize: 1.0,
            font: 0,
            symsize: 1.0,
        }
    }
}
