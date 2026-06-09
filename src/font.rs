//! Font handling: the 14 embedded URW base35 OpenType fonts that stand in for
//! Grace's PostScript font slots, accessed through [`ttf_parser`].
//!
//! Glyph outlines are converted to [`tiny_skia::Path`]s in a 1-em coordinate
//! space (font units divided by units-per-em, Y pointing up), so callers can
//! scale and position them freely.

use ttf_parser::Face;

/// Number of Grace font slots (0..=13).
pub const NUM_FONTS: usize = 14;

/// Embedded OTF data for each Grace font slot, in Grace's canonical AGR font
/// order (the t1lib default used when a file has no `@map font`): bold comes
/// before italic. Verified against QtGrace's rendering of tfonts.agr.
/// Substituted by the metric-compatible URW base35 fonts.
static FONT_DATA: [&[u8]; NUM_FONTS] = [
    include_bytes!("../assets/fonts/NimbusRoman-Regular.otf"), // 0 Times-Roman
    include_bytes!("../assets/fonts/NimbusRoman-Bold.otf"),    // 1 Times-Bold
    include_bytes!("../assets/fonts/NimbusRoman-Italic.otf"),  // 2 Times-Italic
    include_bytes!("../assets/fonts/NimbusRoman-BoldItalic.otf"), // 3 Times-BoldItalic
    include_bytes!("../assets/fonts/NimbusSans-Regular.otf"),  // 4 Helvetica
    include_bytes!("../assets/fonts/NimbusSans-Bold.otf"),     // 5 Helvetica-Bold
    include_bytes!("../assets/fonts/NimbusSans-Italic.otf"),   // 6 Helvetica-Oblique
    include_bytes!("../assets/fonts/NimbusSans-BoldItalic.otf"), // 7 Helvetica-BoldOblique
    include_bytes!("../assets/fonts/NimbusMonoPS-Regular.otf"), // 8 Courier
    include_bytes!("../assets/fonts/NimbusMonoPS-Bold.otf"),   // 9 Courier-Bold
    include_bytes!("../assets/fonts/NimbusMonoPS-Italic.otf"), // 10 Courier-Oblique
    include_bytes!("../assets/fonts/NimbusMonoPS-BoldItalic.otf"), // 11 Courier-BoldOblique
    include_bytes!("../assets/fonts/StandardSymbolsPS.otf"),   // 12 Symbol
    include_bytes!("../assets/fonts/D050000L.otf"),            // 13 ZapfDingbats
];

/// Holds parsed faces for all font slots for the lifetime of a render.
pub struct FontSet {
    faces: Vec<Face<'static>>,
}

impl FontSet {
    /// Parse all embedded fonts. Panics only if a bundled font is corrupt,
    /// which would be a build-time error.
    pub fn load() -> Self {
        let faces = FONT_DATA
            .iter()
            .map(|data| Face::parse(data, 0).expect("bundled font must parse"))
            .collect();
        FontSet { faces }
    }

    /// Borrow the face for a slot, clamping out-of-range indices to slot 0.
    pub fn face(&self, slot: i32) -> &Face<'static> {
        let idx = if (0..NUM_FONTS as i32).contains(&slot) {
            slot as usize
        } else {
            0
        };
        &self.faces[idx]
    }
}

/// A glyph outline as a tiny-skia path in em units (Y up), plus its advance.
pub struct GlyphOutline {
    pub path: Option<tiny_skia::Path>,
    /// Horizontal advance in em units.
    pub advance: f32,
}

/// Collects path segments from `ttf_parser` and scales them into em units.
struct OutlineBuilder {
    builder: tiny_skia::PathBuilder,
    scale: f32,
}

impl ttf_parser::OutlineBuilder for OutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x * self.scale, y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x * self.scale, y * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder
            .quad_to(x1 * self.scale, y1 * self.scale, x * self.scale, y * self.scale);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_to(
            x1 * self.scale,
            y1 * self.scale,
            x2 * self.scale,
            y2 * self.scale,
            x * self.scale,
            y * self.scale,
        );
    }
    fn close(&mut self) {
        self.builder.close();
    }
}

impl FontSet {
    /// Outline a single character in the given font slot, in em units.
    /// Returns a zero-advance empty glyph if the character is missing.
    pub fn outline_char(&self, slot: i32, ch: char) -> GlyphOutline {
        let face = self.face(slot);
        let upem = face.units_per_em() as f32;
        let scale = 1.0 / upem;
        let Some(gid) = face.glyph_index(ch) else {
            return GlyphOutline {
                path: None,
                advance: 0.0,
            };
        };
        let advance = face.glyph_hor_advance(gid).unwrap_or(0) as f32 * scale;
        let mut b = OutlineBuilder {
            builder: tiny_skia::PathBuilder::new(),
            scale,
        };
        face.outline_glyph(gid, &mut b);
        GlyphOutline {
            path: b.builder.finish(),
            advance,
        }
    }

    /// Ascent of a slot in em units (for vertical centering of labels).
    pub fn ascent(&self, slot: i32) -> f32 {
        let face = self.face(slot);
        face.ascender() as f32 / face.units_per_em() as f32
    }

    /// Descent (negative) of a slot in em units.
    pub fn descent(&self, slot: i32) -> f32 {
        let face = self.face(slot);
        face.descender() as f32 / face.units_per_em() as f32
    }
}
