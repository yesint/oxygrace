//! Font handling: the 14 embedded URW base35 OpenType fonts that stand in for
//! Grace's PostScript font slots, accessed through [`ttf_parser`].
//!
//! Glyph outlines are converted to [`tiny_skia::Path`]s in a 1-em coordinate
//! space (font units divided by units-per-em, Y pointing up), so callers can
//! scale and position them freely.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    /// Memoized glyph outlines, keyed by (face slot, char). Outlines are in
    /// size-independent em units, so the key needs no size. `Mutex` (rather
    /// than `RefCell`) keeps the set `Sync`; contention is irrelevant — the
    /// lock cost is dwarfed by CFF charstring interpretation on a miss.
    glyph_cache: Mutex<HashMap<(i32, char), Arc<GlyphOutline>>>,
}

impl FontSet {
    /// Parse all embedded fonts. Panics only if a bundled font is corrupt,
    /// which would be a build-time error.
    pub fn load() -> Self {
        let faces = FONT_DATA
            .iter()
            .map(|data| Face::parse(data, 0).expect("bundled font must parse"))
            .collect();
        FontSet {
            faces,
            glyph_cache: Mutex::new(HashMap::new()),
        }
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

/// Embedded face index of the Symbol font.
pub const FACE_SYMBOL: i32 = 12;

/// Font slot -> embedded face mapping.
///
/// Grace resolves the `@`-file font ids through a mapping set when `@version`
/// is read (pars.yacc): files older than 50001 use the ACE/gr 10-font order
/// (`map_fonts(FONT_MAP_ACEGR)`, t1fonts.cpp) with Symbol at 8; newer files
/// use the font database order (`fonts/FontDataBase`: italic before bold,
/// Courier at 8..11, Symbol at 12). `@map font N to "Name"` overrides single
/// slots.
pub type FontMap = [i32; NUM_FONTS];

/// Default map for files with `@version >= 50001` (FontDataBase order).
pub const FONT_MAP_DEFAULT: FontMap = [0, 2, 1, 3, 4, 6, 5, 7, 8, 10, 9, 11, 12, 13];

/// Map for files with `@version < 50001` (`FONT_MAP_ACEGR`): the old 10-font
/// order, bold before italic, Symbol/Dingbats at 8/9. Slots 10..13 were
/// undefined; they keep identity as a best effort.
pub const FONT_MAP_ACEGR: FontMap = [0, 1, 2, 3, 4, 5, 6, 7, 12, 13, 10, 11, 12, 13];

/// PostScript name of each embedded face, by face index — the inverse of
/// [`face_by_name`], used by the `.agr` writer's `@map font` lines.
pub const FACE_NAMES: [&str; NUM_FONTS] = [
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Courier",
    "Courier-Bold",
    "Courier-Oblique",
    "Courier-BoldOblique",
    "Symbol",
    "ZapfDingbats",
];

/// Resolve a PostScript font name (from `@map font`) to an embedded face.
pub fn face_by_name(name: &str) -> Option<i32> {
    Some(match name {
        "Times-Roman" => 0,
        "Times-Bold" => 1,
        "Times-Italic" => 2,
        "Times-BoldItalic" => 3,
        "Helvetica" => 4,
        "Helvetica-Bold" => 5,
        "Helvetica-Oblique" => 6,
        "Helvetica-BoldOblique" => 7,
        "Courier" => 8,
        "Courier-Bold" => 9,
        "Courier-Oblique" => 10,
        "Courier-BoldOblique" => 11,
        "Symbol" => 12,
        "ZapfDingbats" => 13,
        _ => return None,
    })
}

/// Map an 8-bit character code to the Unicode character a *face* renders.
/// Text faces use Latin-1 (code = codepoint). The Symbol face goes through
/// the Adobe Symbol encoding, so `a` is alpha, `D` Delta, 0xB4 the multiply
/// sign — Grace addresses Symbol by 8-bit codes (`\x`, `\c`). The bundled
/// StandardSymbolsPS cmap covers those same 8-bit codes directly, so the
/// identity is used there; the table is kept for fonts with Unicode cmaps.
pub fn map_font_char(face: i32, code: u32) -> char {
    // All bundled faces, including StandardSymbolsPS (whose cmap covers the
    // Adobe Symbol 8-bit codes natively: 20-7e, 80, a0-ef, f1-fe), accept
    // the 8-bit code as the codepoint, i.e. Latin-1 semantics.
    let _ = face;
    char::from_u32(code).unwrap_or('\u{FFFD}')
}

/// Transliterate one Adobe-Symbol-encoded character to the Unicode
/// character it *displays* as (Greek letters; other codes pass through
/// unchanged). The renderer never needs this — StandardSymbolsPS maps the
/// 8-bit codes through its own cmap — but UI labels (GUI tree, status bar)
/// need real codepoints to show `\xa` as an alpha.
pub fn symbol_to_unicode(c: char) -> char {
    match c {
        'A' => 'Α', 'B' => 'Β', 'G' => 'Γ', 'D' => 'Δ', 'E' => 'Ε', 'Z' => 'Ζ',
        'H' => 'Η', 'Q' => 'Θ', 'I' => 'Ι', 'K' => 'Κ', 'L' => 'Λ', 'M' => 'Μ',
        'N' => 'Ν', 'X' => 'Ξ', 'O' => 'Ο', 'P' => 'Π', 'R' => 'Ρ', 'S' => 'Σ',
        'T' => 'Τ', 'U' => 'Υ', 'F' => 'Φ', 'C' => 'Χ', 'Y' => 'Ψ', 'W' => 'Ω',
        'a' => 'α', 'b' => 'β', 'g' => 'γ', 'd' => 'δ', 'e' => 'ε', 'z' => 'ζ',
        'h' => 'η', 'q' => 'θ', 'i' => 'ι', 'k' => 'κ', 'l' => 'λ', 'm' => 'μ',
        'n' => 'ν', 'x' => 'ξ', 'o' => 'ο', 'p' => 'π', 'r' => 'ρ', 's' => 'σ',
        't' => 'τ', 'u' => 'υ', 'f' => 'φ', 'c' => 'χ', 'y' => 'ψ', 'w' => 'ω',
        'J' => 'ϑ', 'j' => 'ϕ', 'V' => 'ς', 'v' => 'ϖ',
        // Operators/relations from the Symbol high codes (which differ
        // from Latin-1 at these positions).
        '\u{22}' => '∀', '\u{24}' => '∃', '\u{27}' => '∋', '\u{7E}' => '∼',
        '\u{A2}' => '′', '\u{A3}' => '≤', '\u{A5}' => '∞', '\u{AB}' => '↔',
        '\u{AC}' => '←', '\u{AD}' => '↑', '\u{AE}' => '→', '\u{AF}' => '↓',
        '\u{B3}' => '≥', '\u{B4}' => '×', '\u{B5}' => '∝', '\u{B6}' => '∂',
        '\u{B7}' => '•', '\u{B8}' => '÷', '\u{B9}' => '≠', '\u{BA}' => '≡',
        '\u{BB}' => '≈', '\u{BC}' => '…', '\u{C0}' => 'ℵ', '\u{C1}' => 'ℑ',
        '\u{C2}' => 'ℜ', '\u{C3}' => '℘', '\u{C4}' => '⊗', '\u{C5}' => '⊕',
        '\u{C6}' => '∅', '\u{C7}' => '∩', '\u{C8}' => '∪', '\u{C9}' => '⊃',
        '\u{CA}' => '⊇', '\u{CC}' => '⊂', '\u{CD}' => '⊆', '\u{CE}' => '∈',
        '\u{CF}' => '∉', '\u{D0}' => '∠', '\u{D1}' => '∇', '\u{D5}' => '∏',
        '\u{D6}' => '√', '\u{D7}' => '·', '\u{D8}' => '¬', '\u{D9}' => '∧',
        '\u{DA}' => '∨', '\u{DB}' => '⇔', '\u{DC}' => '⇐', '\u{DD}' => '⇑',
        '\u{DE}' => '⇒', '\u{DF}' => '⇓', '\u{E1}' => '⟨', '\u{E5}' => '∑',
        '\u{F1}' => '⟩', '\u{F2}' => '∫', '\\' => '∴',
        _ => c,
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
    /// Outline a single character in the given font slot, in em units,
    /// memoized for the lifetime of the set. Returns a zero-advance empty
    /// glyph if the character is missing.
    pub fn outline_char(&self, slot: i32, ch: char) -> Arc<GlyphOutline> {
        // Clamp like `face()` so all out-of-range slots share one cache entry.
        let slot = if (0..NUM_FONTS as i32).contains(&slot) { slot } else { 0 };
        if let Some(hit) = self.glyph_cache.lock().unwrap().get(&(slot, ch)) {
            return Arc::clone(hit);
        }
        let outline = Arc::new(self.build_outline(slot, ch));
        self.glyph_cache
            .lock()
            .unwrap()
            .insert((slot, ch), Arc::clone(&outline));
        outline
    }

    /// Build a glyph outline uncached (the cache-miss path).
    fn build_outline(&self, slot: i32, ch: char) -> GlyphOutline {
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

    /// Glyph ink bounding box in em units `(x_min, y_min, x_max, y_max)`,
    /// Y up with the baseline at 0. `None` for missing/empty glyphs (spaces).
    pub fn glyph_bbox(&self, slot: i32, ch: char) -> Option<(f32, f32, f32, f32)> {
        let face = self.face(slot);
        let gid = face.glyph_index(ch)?;
        let r = face.glyph_bounding_box(gid)?;
        let s = 1.0 / face.units_per_em() as f32;
        Some((
            r.x_min as f32 * s,
            r.y_min as f32 * s,
            r.x_max as f32 * s,
            r.y_max as f32 * s,
        ))
    }

    /// Ascent of a slot in em units (for vertical centering of labels).
    pub fn ascent(&self, slot: i32) -> f32 {
        let face = self.face(slot);
        face.ascender() as f32 / face.units_per_em() as f32
    }

    /// Underline position (negative, below baseline) and thickness in em
    /// units, from the font's own metrics.
    pub fn underline_metrics(&self, slot: i32) -> (f32, f32) {
        let face = self.face(slot);
        let upem = face.units_per_em() as f32;
        match face.underline_metrics() {
            Some(m) => (m.position as f32 / upem, m.thickness as f32 / upem),
            None => (-0.1, 0.05),
        }
    }

    /// Descent (negative) of a slot in em units.
    pub fn descent(&self, slot: i32) -> f32 {
        let face = self.face(slot);
        face.descender() as f32 / face.units_per_em() as f32
    }
}
