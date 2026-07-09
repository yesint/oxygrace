//! Grace string markup parsing and layout.
//!
//! Grace strings embed formatting escapes introduced by `\`. This module is a
//! literal port of the escape semantics of QtGrace's `WriteString`
//! (`t1fonts.cpp`): per-character state of font, size scale, baseline /
//! vertical shift, color, upper-half charset, under/overline, plus pen marks
//! and explicit shifts. The laid-out glyph list (em units of the base size)
//! is consumed by the canvas, which positions the whole block by its rendered
//! bounding box.

use crate::font::{FontMap, FontSet};

/// `SSCRIPT_SCALE` (t1fonts.h): size factor applied by `\s`/`\S`.
const SSCRIPT_SCALE: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// `SUBSCRIPT_SHIFT` / `SUPSCRIPT_SHIFT` (t1fonts.h).
const SUBSCRIPT_SHIFT: f32 = 0.4;
const SUPSCRIPT_SHIFT: f32 = 0.6;
/// `ENLARGE_SCALE` = sqrt(sqrt(2)) (t1fonts.h): factor for `\+` / `\-`.
const ENLARGE_SCALE: f32 = 1.189_207_1;

/// A 2x2 text matrix (Grace `TextMatrix`, t1fonts.cpp): glyph outlines are
/// transformed by it and the pen advances by `tm * (advance, 0)`, which is
/// how rotated/mirrored/slanted text (`\t \T \r \l \q`) works.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tm {
    pub xx: f32,
    pub xy: f32,
    pub yx: f32,
    pub yy: f32,
}

impl Tm {
    pub const UNIT: Tm = Tm { xx: 1.0, xy: 0.0, yx: 0.0, yy: 1.0 };

    fn det(&self) -> f32 {
        self.xx * self.yy - self.xy * self.yx
    }

    /// Grace `tm_size`: the effective character size of the matrix
    /// (signed; mirrored matrices have negative determinants).
    pub fn size(&self) -> f32 {
        self.det() / (self.xx * self.xx + self.yx * self.yx).sqrt()
    }

    /// Left-multiply by `p` (Grace `tm_product`; no-op if `p` is singular).
    fn product(&mut self, p: Tm) {
        if p.det() == 0.0 {
            return;
        }
        *self = Tm {
            xx: p.xx * self.xx + p.xy * self.yx,
            xy: p.xx * self.xy + p.xy * self.yy,
            yx: p.yx * self.xx + p.yy * self.yx,
            yy: p.yx * self.xy + p.yy * self.yy,
        };
    }

    fn scale(&mut self, s: f32) {
        self.xx *= s;
        self.xy *= s;
        self.yx *= s;
        self.yy *= s;
    }

    fn rotate(&mut self, angle_deg: f32) {
        if angle_deg != 0.0 {
            let (si, co) = (angle_deg.to_radians().sin(), angle_deg.to_radians().cos());
            self.product(Tm { xx: co, xy: -si, yx: si, yy: co });
        }
    }

    fn slant(&mut self, s: f32) {
        if s != 0.0 {
            self.product(Tm { xx: 1.0, xy: s, yx: 0.0, yy: 1.0 });
        }
    }

    /// Apply to a point (em units, Y up).
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.xx * x + self.xy * y, self.yx * x + self.yy * y)
    }
}

/// A run of text sharing one style.
#[derive(Debug, Clone)]
pub struct StyledRun {
    pub text: String,
    /// Font slot (0..=13).
    pub font: i32,
    /// Text matrix (size, rotation, mirror, slant combined).
    pub tm: Tm,
    /// Baseline shift in em units of the base size (positive = up),
    /// applied along the matrix's vertical column.
    pub baseline: f32,
    /// Optional per-run color override (color index).
    pub color: Option<i32>,
    pub underline: bool,
    pub overline: bool,
}

/// One parsed item: a styled text run or a pen-motion control.
#[derive(Debug, Clone)]
enum Item {
    Run(StyledRun),
    /// One-shot pen shift of this many em along the matrix's horizontal
    /// column (`\h{}`; WriteString `hvpshift`). Carries the matrix so the
    /// shift direction follows rotations.
    HShift(f32, Tm),
    /// Remember the current pen position as mark `n` (`\m{n}`).
    SetMark(i32),
    /// Return the pen to mark `n` (`\M{n}`).
    GotoMark(i32),
    /// Line break (`\n`): pen returns to the line start, baseline drops 1 em.
    Newline,
}

/// Parse Grace markup into items, starting from a base font *slot*; the
/// produced runs carry resolved *face* indices via `map`.
fn parse_items(input: &str, base_font: i32, map: &FontMap) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    // `font` always holds a resolved *face*: numeric selections go through
    // the slot map (get_mapped_font), names and \x resolve directly by name
    // (get_font_by_name), exactly as WriteString does.
    let mut font = resolve_face(base_font, map);
    let mut tm = Tm::UNIT;
    // Current vertical shift and the "baseline" that \v{} / \N return to
    // (WriteString's `vshift` and `baseline`, in base-em units).
    let mut vshift = 0.0f32;
    let mut baseline = 0.0f32;
    let mut color: Option<i32> = None;
    let mut upperset = false;
    let mut underline = false;
    let mut overline = false;
    let mut buf = String::new();

    macro_rules! flushcur {
        () => {
            if !buf.is_empty() {
                items.push(Item::Run(StyledRun {
                    text: std::mem::take(&mut buf),
                    font,
                    tm,
                    baseline: vshift,
                    color,
                    underline,
                    overline,
                }));
            }
        };
    }

    // Push a possibly upper-half character, translating Symbol-font input
    // through the Adobe Symbol encoding.
    let push_char = |buf: &mut String, font: i32, upperset: bool, c: char| {
        let code = if upperset && (c as u32) < 0x80 {
            c as u32 + 0x80
        } else {
            c as u32
        };
        buf.push(crate::font::map_font_char(font, code));
    };

    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            push_char(&mut buf, font, upperset, c);
            continue;
        }
        let Some(&next) = chars.peek() else { break };
        match next {
            '\\' => {
                push_char(&mut buf, font, upperset, '\\');
                chars.next();
            }
            // Font selection by single digit: \0..\9
            '0'..='9' => {
                flushcur!();
                font = resolve_face(next.to_digit(10).unwrap() as i32, map);
                chars.next();
            }
            // Escapes taking a {...} argument.
            'f' | 'R' | 'z' | 'Z' | 'v' | 'V' | 'h' | 'm' | 'M' | '#' | 'r' | 'l' | 't' | 'T' => {
                let kind = next;
                chars.next();
                let mut arg = String::new();
                let mut had_braces = false;
                if chars.peek() == Some(&'{') {
                    had_braces = true;
                    chars.next();
                    for ac in chars.by_ref() {
                        if ac == '}' {
                            break;
                        }
                        arg.push(ac);
                    }
                }
                let a = arg.trim();
                match kind {
                    'f' => {
                        flushcur!();
                        font = if a.is_empty() {
                            resolve_face(base_font, map)
                        } else if let Ok(n) = a.parse::<i32>() {
                            resolve_face(n, map)
                        } else {
                            // Font names address the face directly.
                            crate::font::face_by_name(a)
                                .unwrap_or_else(|| resolve_face(base_font, map))
                        };
                    }
                    'R' => {
                        flushcur!();
                        // Numeric index or a color name (WriteString falls
                        // back to get_color_by_name); \R{} resets.
                        color = a
                            .parse::<i32>()
                            .ok()
                            .or_else(|| crate::color::index_by_name(a));
                    }
                    // \z{x} multiplies the size; \z{} resets it to 1
                    // keeping the orientation (t1fonts.cpp).
                    'z' => {
                        flushcur!();
                        if a.is_empty() {
                            let sz = tm.size();
                            if sz != 0.0 {
                                tm.scale(1.0 / sz);
                            }
                        } else if let Ok(v) = a.parse::<f32>() {
                            tm.scale(v);
                        }
                    }
                    // \Z{x} sets the absolute size factor.
                    'Z' => {
                        flushcur!();
                        if let Ok(v) = a.parse::<f32>() {
                            let sz = tm.size();
                            if sz != 0.0 {
                                tm.scale(v / sz);
                            }
                        }
                    }
                    // \v{x}: shift up by x of the current size; \v{} returns
                    // to the baseline. \V also moves the baseline itself.
                    'v' => {
                        flushcur!();
                        if a.is_empty() {
                            vshift = baseline;
                        } else if let Ok(v) = a.parse::<f32>() {
                            vshift += tm.size() * v;
                        }
                    }
                    'V' => {
                        flushcur!();
                        if a.is_empty() {
                            baseline = 0.0;
                        } else if let Ok(v) = a.parse::<f32>() {
                            baseline += tm.size() * v;
                        }
                        vshift = baseline;
                    }
                    'h' => {
                        flushcur!();
                        if let Ok(v) = a.parse::<f32>() {
                            items.push(Item::HShift(v, tm));
                        }
                    }
                    // Text-matrix escapes: \r rotate, \l slant, \t multiply
                    // (or reset with no argument), \T set absolute.
                    'r' => {
                        flushcur!();
                        if let Ok(v) = a.parse::<f32>() {
                            tm.rotate(v);
                        }
                    }
                    'l' => {
                        flushcur!();
                        if let Ok(v) = a.parse::<f32>() {
                            tm.slant(v);
                        }
                    }
                    't' | 'T' => {
                        flushcur!();
                        let nums: Vec<f32> =
                            a.split_whitespace().filter_map(|w| w.parse().ok()).collect();
                        if kind == 't' && a.is_empty() {
                            tm = Tm::UNIT;
                        } else if let [xx, xy, yx, yy] = nums[..] {
                            let p = Tm { xx, xy, yx, yy };
                            if kind == 'T' {
                                tm = p;
                            } else {
                                tm.product(p);
                            }
                        }
                    }
                    'm' => {
                        flushcur!();
                        if let Ok(n) = a.parse::<i32>() {
                            items.push(Item::SetMark(n));
                        }
                    }
                    'M' => {
                        flushcur!();
                        if let Ok(n) = a.parse::<i32>() {
                            items.push(Item::GotoMark(n));
                            vshift = baseline;
                        }
                    }
                    // \#{ab12} inserts raw Latin-1 bytes by hex code.
                    '#' => {
                        let bytes: Vec<u32> = a
                            .as_bytes()
                            .chunks(2)
                            .filter(|ch| ch.len() == 2)
                            .filter_map(|ch| {
                                u32::from_str_radix(std::str::from_utf8(ch).ok()?, 16).ok()
                            })
                            .collect();
                        for b in bytes {
                            buf.push(crate::font::map_font_char(resolve_face(font, map), b));
                        }
                    }
                    _ => {
                        let _ = had_braces;
                    }
                }
            }
            'x' => {
                flushcur!();
                font = crate::font::FACE_SYMBOL;
                chars.next();
            }
            // Subscript: shift down 0.4 of the *current* size, then shrink
            // (both cumulative, per WriteString).
            's' => {
                flushcur!();
                vshift -= tm.size() * SUBSCRIPT_SHIFT;
                tm.scale(SSCRIPT_SCALE);
                chars.next();
            }
            'S' => {
                flushcur!();
                vshift += tm.size() * SUPSCRIPT_SHIFT;
                tm.scale(SSCRIPT_SCALE);
                chars.next();
            }
            // Return to size 1 (keeping orientation) on the current baseline.
            'N' => {
                flushcur!();
                let sz = tm.size();
                if sz != 0.0 {
                    tm.scale(1.0 / sz);
                }
                vshift = baseline;
                chars.next();
            }
            '+' => {
                flushcur!();
                tm.scale(ENLARGE_SCALE);
                chars.next();
            }
            '-' => {
                flushcur!();
                tm.scale(1.0 / ENLARGE_SCALE);
                chars.next();
            }
            // Oblique on/off: slant by +-OBLIQUE_FACTOR (t1fonts.h 0.25).
            'q' => {
                flushcur!();
                tm.slant(0.25);
                chars.next();
            }
            'Q' => {
                flushcur!();
                tm.slant(-0.25);
                chars.next();
            }
            // Upper-half charset on/off (\c .. \C).
            'c' => {
                flushcur!();
                upperset = true;
                chars.next();
            }
            'C' => {
                flushcur!();
                upperset = false;
                chars.next();
            }
            'u' => {
                flushcur!();
                underline = true;
                chars.next();
            }
            'U' => {
                flushcur!();
                underline = false;
                chars.next();
            }
            'o' => {
                flushcur!();
                overline = true;
                chars.next();
            }
            'O' => {
                flushcur!();
                overline = false;
                chars.next();
            }
            // Line break: baseline drops exactly one em, pen returns to the
            // line start (t1fonts.cpp case 'n': baseline -= 1.0, MARK_CR).
            'n' => {
                flushcur!();
                baseline -= 1.0;
                vshift = baseline;
                items.push(Item::Newline);
                chars.next();
            }
            // \B: revert to the base font only (font/charset, not size).
            'B' => {
                flushcur!();
                font = resolve_face(base_font, map);
                chars.next();
            }
            // Unknown single-char escape: drop the marker char.
            _ => {
                chars.next();
            }
        }
    }
    flushcur!();
    items
}

/// Flatten Grace markup to the plain Unicode text it displays as: escapes
/// are interpreted and dropped, `\n` becomes a space, and Symbol-font runs
/// are transliterated to Greek. Not a renderer — the canvas draws markup
/// via [`layout`]; this is for surfaces (GUI tree, status bar) that can
/// only show an ordinary string.
pub fn plain(input: &str, base_font: i32, map: &FontMap) -> String {
    let mut out = String::new();
    for item in parse_items(input, base_font, map) {
        match item {
            Item::Run(r) if r.font == crate::font::FACE_SYMBOL => {
                out.extend(r.text.chars().map(crate::font::symbol_to_unicode));
            }
            Item::Run(r) => out.push_str(&r.text),
            Item::Newline => out.push(' '),
            _ => {}
        }
    }
    out
}

/// Parse Grace markup into styled runs (motion controls dropped) — kept for
/// callers that only need the styling, e.g. tests.
pub fn parse(input: &str, base_font: i32, map: &FontMap) -> Vec<StyledRun> {
    parse_items(input, base_font, map)
        .into_iter()
        .filter_map(|i| match i {
            Item::Run(r) => Some(r),
            _ => None,
        })
        .collect()
}

/// Resolve a font slot to an embedded face through the project map.
fn resolve_face(slot: i32, map: &FontMap) -> i32 {
    if (0..map.len() as i32).contains(&slot) {
        map[slot as usize]
    } else {
        slot
    }
}


/// A glyph positioned in the text frame, in em units of the base size.
pub struct LaidGlyph {
    pub font: i32,
    /// Pen position of the glyph origin (em units; rotated/mirrored runs
    /// advance the pen in both coordinates).
    pub x: f32,
    pub y: f32,
    /// Text matrix applied to the outline.
    pub tm: Tm,
    pub ch: char,
    pub color: Option<i32>,
}

/// An under/overline rule in em units: from `x0` to `x1` at height `y`,
/// `thickness` thick.
pub struct Rule {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
    pub thickness: f32,
    pub color: Option<i32>,
}

/// Result of laying out a marked-up string: positioned glyphs plus total width.
pub struct Layout {
    pub glyphs: Vec<LaidGlyph>,
    /// Underline/overline rules.
    pub rules: Vec<Rule>,
    /// Total advance width in em units of the base size (widest line).
    pub width: f32,
}

/// Lay out a marked-up string into positioned glyphs (em units, base size 1).
pub fn layout(fonts: &FontSet, input: &str, base_font: i32, map: &FontMap) -> Layout {
    let items = parse_items(input, base_font, map);
    let mut glyphs = Vec::new();
    let mut rules: Vec<Rule> = Vec::new();
    // The pen moves in 2D: rotated/mirrored matrices advance it along the
    // transformed baseline direction (WriteString: rpoint += glyph advance).
    let (mut px, mut py) = (0.0f32, 0.0f32);
    let mut width = 0.0f32;
    let mut marks: std::collections::HashMap<i32, (f32, f32)> = std::collections::HashMap::new();
    for item in &items {
        match item {
            // hvpshift: hshift = size*val applied along (cxx, cyx)/size —
            // net val along the matrix's first column (WriteString).
            Item::HShift(h, tm) => {
                px += tm.xx * h;
                py += tm.yx * h;
            }
            Item::SetMark(n) => {
                marks.insert(*n, (px, py));
            }
            Item::GotoMark(n) => {
                if let Some(&(mx, my)) = marks.get(n) {
                    (px, py) = (mx, my);
                }
            }
            Item::Newline => {
                width = width.max(px);
                (px, py) = (0.0, 0.0);
            }
            Item::Run(run) => {
                // vvpshift: the baseline shift acts along the matrix's
                // vertical column (WriteString: (cxy, cyy)*vshift/size).
                let sz = run.tm.size();
                let (vsx, vsy) = if sz != 0.0 {
                    (
                        run.tm.xy * run.baseline / sz,
                        run.tm.yy * run.baseline / sz,
                    )
                } else {
                    (0.0, run.baseline)
                };
                let (startx, starty) = (px, py);
                for ch in run.text.chars() {
                    let g = fonts.outline_char(run.font, ch);
                    glyphs.push(LaidGlyph {
                        font: run.font,
                        x: px + vsx,
                        y: py + vsy,
                        tm: run.tm,
                        ch,
                        color: run.color,
                    });
                    // Pen advance through the matrix.
                    let (ax, ay) = run.tm.apply(g.advance, 0.0);
                    px += ax;
                    py += ay;
                }
                if (run.underline || run.overline) && px > startx {
                    // Rule geometry from the font's metrics, scaled with the
                    // run size (Grace's t1lib draws these from the metrics);
                    // drawn along the plain baseline (the demo's underlined
                    // strings use the unit matrix).
                    let scale = run.tm.size().abs();
                    let (upos, uthick) = fonts.underline_metrics(run.font);
                    if run.underline {
                        rules.push(Rule {
                            x0: startx + vsx,
                            x1: px + vsx,
                            y: starty + vsy + upos * scale,
                            thickness: uthick * scale,
                            color: run.color,
                        });
                    }
                    if run.overline {
                        let asc = fonts.ascent(run.font);
                        rules.push(Rule {
                            x0: startx + vsx,
                            x1: px + vsx,
                            y: starty + vsy + (asc + uthick) * scale,
                            thickness: uthick * scale,
                            color: run.color,
                        });
                    }
                }
            }
        }
    }
    Layout {
        glyphs,
        rules,
        width: width.max(px),
    }
}

/// Quick measurement of a marked-up string's advance width in em units.
pub fn measure(fonts: &FontSet, input: &str, base_font: i32, map: &FontMap) -> f32 {
    layout(fonts, input, base_font, map).width
}

/// Rendered ink bounding box of a marked-up string, in em units of the base
/// size: `(x_min, y_min, x_max, y_max)`, baseline-left origin, Y up. This is
/// the union of the positioned glyph outlines (mirrors Grace's `update_bbox`),
/// so it accounts for actual glyph extents, ascenders/descenders, and the
/// baseline shifts and scaling from sub/superscript markup. `None` if no glyph
/// has an outline (e.g. all spaces or an empty string).
pub fn bbox(fonts: &FontSet, input: &str, base_font: i32, map: &FontMap) -> Option<(f32, f32, f32, f32)> {
    let layout = layout(fonts, input, base_font, map);
    let mut found = false;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for g in &layout.glyphs {
        if let Some((gx0, gy0, gx1, gy1)) = fonts.glyph_bbox(g.font, g.ch) {
            // Transform all four corners through the run's text matrix.
            for &(cx, cy) in &[(gx0, gy0), (gx1, gy0), (gx1, gy1), (gx0, gy1)] {
                let (tx, ty) = g.tm.apply(cx, cy);
                x0 = x0.min(g.x + tx);
                x1 = x1.max(g.x + tx);
                y0 = y0.min(g.y + ty);
                y1 = y1.max(g.y + ty);
            }
            found = true;
        }
    }
    found.then_some((x0, y0, x1, y1))
}

#[cfg(test)]
mod tests {
    use crate::font::FONT_MAP_DEFAULT;

    /// `plain` drops escapes, keeps literal text, transliterates Symbol
    /// runs to Greek and turns `\n` into a space.
    #[test]
    fn plain_flattens_markup() {
        let p = |s: &str| super::plain(s, 0, &FONT_MAP_DEFAULT);
        assert_eq!(p(r"CO\s2\N (ppm)"), "CO2 (ppm)");
        assert_eq!(p(r"original: \xa\0=30°"), "original: α=30°");
        assert_eq!(p(r"\f{Courier-Bold}BarDY\f{} type"), "BarDY type");
        assert_eq!(p("line one\\nline two"), "line one line two");
        assert_eq!(p(r"\xDW\f{}"), "ΔΩ");
    }
}
