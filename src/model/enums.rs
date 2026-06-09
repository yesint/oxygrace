//! Enumerations mirroring Grace's internal type codes.
//!
//! The numeric values match the constants used by Grace/QtGrace (see
//! `src/defines.h` and `src/graphs.h` in the QtGrace6 reference) so that the
//! integer codes appearing in `.agr` files map directly onto these variants.

/// Kind of graph. Only [`GraphType::Xy`] is rendered in milestone 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphType {
    #[default]
    Xy,
    Chart,
    Polar,
    Smith,
    Fixed,
    Pie,
    Polar2,
}

/// Dataset column layout. The discriminant matches Grace's `SetType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SetType {
    #[default]
    Xy,
    XyDx,
    XyDy,
    XyDxDx,
    XyDyDy,
    XyDxDy,
    XyDxDxDyDy,
    Bar,
    BarDy,
    BarDyDy,
    XyHiLo,
    Xyz,
    XyR,
    XySize,
    XyColor,
    XyColPat,
    XyVMap,
    BoxPlot,
    Band,
}

impl SetType {
    /// Number of numeric data columns this dataset type consumes.
    pub fn ncols(self) -> usize {
        use SetType::*;
        match self {
            Xy | Bar => 2,
            XyDx | XyDy | Xyz | XyR | XySize | XyColor | XyColPat | XyVMap | BarDy => 3,
            XyDxDx | XyDyDy | XyDxDy | BarDyDy => 4,
            XyHiLo => 5,
            XyDxDxDyDy | BoxPlot => 6,
            Band => 2,
        }
    }

    /// Parse the `@type` keyword (case-insensitive) used in `.agr` files.
    pub fn parse(s: &str) -> Option<Self> {
        use SetType::*;
        Some(match s.to_ascii_lowercase().as_str() {
            "xy" => Xy,
            "xydx" => XyDx,
            "xydy" => XyDy,
            "xydxdx" => XyDxDx,
            "xydydy" => XyDyDy,
            "xydxdy" => XyDxDy,
            "xydxdxdydy" => XyDxDxDyDy,
            "bar" => Bar,
            "bardy" => BarDy,
            "bardydy" => BarDyDy,
            "xyhilo" => XyHiLo,
            "xyz" => Xyz,
            "xyr" => XyR,
            "xysize" => XySize,
            "xycolor" => XyColor,
            "xycolpat" => XyColPat,
            "xyvmap" => XyVMap,
            "boxplot" => BoxPlot,
            "band" => Band,
            _ => return None,
        })
    }
}

/// Axis scale mapping. Only [`ScaleType::Normal`] is rendered in milestone 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleType {
    #[default]
    Normal,
    Logarithmic,
    Reciprocal,
    Logit,
}

/// Plot symbol kind (`s<n> symbol <code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymbolType {
    #[default]
    None,
    Circle,
    Square,
    Diamond,
    TriangleUp,
    TriangleLeft,
    TriangleDown,
    TriangleRight,
    Plus,
    Cross,
    Star,
    Char,
}

impl SymbolType {
    /// Map Grace's integer symbol code to a [`SymbolType`].
    pub fn from_code(code: i32) -> Self {
        use SymbolType::*;
        match code {
            1 => Circle,
            2 => Square,
            3 => Diamond,
            4 => TriangleUp,
            5 => TriangleLeft,
            6 => TriangleDown,
            7 => TriangleRight,
            8 => Plus,
            9 => Cross,
            10 => Star,
            11 => Char,
            _ => None,
        }
    }
}

/// How successive data points are connected (`s<n> line type <code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineType {
    None,
    #[default]
    Straight,
    LeftStair,
    RightStair,
    Segment2,
    Segment3,
    IncrX,
    DecrX,
}

impl LineType {
    /// Map Grace's integer line-type code to a [`LineType`].
    pub fn from_code(code: i32) -> Self {
        use LineType::*;
        match code {
            0 => None,
            1 => Straight,
            2 => LeftStair,
            3 => RightStair,
            4 => Segment2,
            5 => Segment3,
            6 => IncrX,
            7 => DecrX,
            _ => Straight,
        }
    }
}

/// Set fill mode (`s<n> fill type <code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillType {
    #[default]
    None,
    Polygon,
    Baseline,
}

/// Numeric tick-label format. Only [`TickFormat::Decimal`] and
/// [`TickFormat::General`] are formatted in milestone 1; others fall back to a
/// general representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickFormat {
    #[default]
    Decimal,
    Exponential,
    General,
    Power,
    Scientific,
    Engineering,
}

impl TickFormat {
    /// Parse the `ticklabel format <kw>` keyword.
    pub fn parse(s: &str) -> Option<Self> {
        use TickFormat::*;
        Some(match s.to_ascii_lowercase().as_str() {
            "decimal" => Decimal,
            "exponential" => Exponential,
            "general" => General,
            "power" => Power,
            "scientific" => Scientific,
            "engineering" => Engineering,
            _ => return None,
        })
    }
}

/// Where ticks / labels are placed relative to the axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    Normal,
    Opposite,
    Both,
}

/// Which of the four per-graph axes a tickmark block configures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisId {
    X,
    Y,
    AltX,
    AltY,
}

impl AxisId {
    /// Index into `Graph::axes`.
    pub fn index(self) -> usize {
        match self {
            AxisId::X => 0,
            AxisId::Y => 1,
            AxisId::AltX => 2,
            AxisId::AltY => 3,
        }
    }

    /// True for the two horizontal axes.
    pub fn is_x(self) -> bool {
        matches!(self, AxisId::X | AxisId::AltX)
    }
}
