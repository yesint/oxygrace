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

/// A run of text sharing one style.
#[derive(Debug, Clone)]
pub struct StyledRun {
    pub text: String,
    /// Font slot (0..=13).
    pub font: i32,
    /// Size multiplier relative to the base size.
    pub scale: f32,
    /// Baseline shift in em units of the base size (positive = up).
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
    /// One-shot horizontal pen shift, in em units of the base size (`\h{}`).
    HShift(f32),
    /// Remember the current pen x as mark `n` (`\m{n}`).
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
    let mut scale = 1.0f32;
    // Current vertical shift and the "baseline" that \v{} / \N return to
    // (WriteString's `vshift` and `baseline`, in base-em units).
    let mut vshift = 0.0f32;
    let mut baseline = 0.0f32;
    let mut color: Option<i32> = None;
    let mut upperset = false;
    let mut underline = false;
    let mut overline = false;
    let mut buf = String::new();

    macro_rules! flush {
        ($font:expr, $scale:expr, $vshift:expr) => {
            if !buf.is_empty() {
                items.push(Item::Run(StyledRun {
                    text: std::mem::take(&mut buf),
                    font: $font,
                    scale: $scale,
                    baseline: $vshift,
                    color,
                    underline,
                    overline,
                }));
            }
        };
    }
    macro_rules! flushcur {
        () => {
            flush!(font, scale, vshift)
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
                    // \z{x} multiplies the size; \z{} resets it (t1fonts.cpp).
                    'z' => {
                        flushcur!();
                        if a.is_empty() {
                            scale = 1.0;
                        } else if let Ok(v) = a.parse::<f32>() {
                            scale *= v;
                        }
                    }
                    // \Z{x} sets the absolute size factor.
                    'Z' => {
                        flushcur!();
                        if let Ok(v) = a.parse::<f32>() {
                            scale = v;
                        }
                    }
                    // \v{x}: shift up by x of the current size; \v{} returns
                    // to the baseline. \V also moves the baseline itself.
                    'v' => {
                        flushcur!();
                        if a.is_empty() {
                            vshift = baseline;
                        } else if let Ok(v) = a.parse::<f32>() {
                            vshift += scale * v;
                        }
                    }
                    'V' => {
                        flushcur!();
                        if a.is_empty() {
                            baseline = 0.0;
                        } else if let Ok(v) = a.parse::<f32>() {
                            baseline += scale * v;
                        }
                        vshift = baseline;
                    }
                    'h' => {
                        flushcur!();
                        if let Ok(v) = a.parse::<f32>() {
                            items.push(Item::HShift(scale * v));
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
                    // Transform escapes (\r rotate, \l slant, \t, \T) are not
                    // supported; their argument is consumed and ignored.
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
                vshift -= scale * SUBSCRIPT_SHIFT;
                scale *= SSCRIPT_SCALE;
                chars.next();
            }
            'S' => {
                flushcur!();
                vshift += scale * SUPSCRIPT_SHIFT;
                scale *= SSCRIPT_SCALE;
                chars.next();
            }
            // Return to normal size on the current baseline.
            'N' => {
                flushcur!();
                scale = 1.0;
                vshift = baseline;
                chars.next();
            }
            '+' => {
                flushcur!();
                scale *= ENLARGE_SCALE;
                chars.next();
            }
            '-' => {
                flushcur!();
                scale /= ENLARGE_SCALE;
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
    flush!(font, scale, vshift);
    items
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


/// A glyph positioned along the text baseline, in em units of the base size.
pub struct LaidGlyph {
    pub font: i32,
    /// Horizontal pen position of the glyph origin (em units).
    pub x: f32,
    /// Baseline offset (em units, positive up).
    pub y: f32,
    pub scale: f32,
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
    let mut pen = 0.0f32;
    let mut width = 0.0f32;
    let mut marks: std::collections::HashMap<i32, f32> = std::collections::HashMap::new();
    for item in &items {
        match item {
            Item::HShift(dx) => pen += dx,
            Item::SetMark(n) => {
                marks.insert(*n, pen);
            }
            Item::GotoMark(n) => {
                if let Some(&x) = marks.get(n) {
                    pen = x;
                }
            }
            Item::Newline => {
                width = width.max(pen);
                pen = 0.0;
            }
            Item::Run(run) => {
                let start = pen;
                for ch in run.text.chars() {
                    let g = fonts.outline_char(run.font, ch);
                    glyphs.push(LaidGlyph {
                        font: run.font,
                        x: pen,
                        y: run.baseline,
                        scale: run.scale,
                        ch,
                        color: run.color,
                    });
                    pen += g.advance * run.scale;
                }
                if (run.underline || run.overline) && pen > start {
                    // Rule geometry from the font's metrics, scaled with the
                    // run (Grace's t1lib draws these from the same metrics).
                    let (upos, uthick) = fonts.underline_metrics(run.font);
                    if run.underline {
                        rules.push(Rule {
                            x0: start,
                            x1: pen,
                            y: run.baseline + upos * run.scale,
                            thickness: uthick * run.scale,
                            color: run.color,
                        });
                    }
                    if run.overline {
                        let asc = fonts.ascent(run.font);
                        rules.push(Rule {
                            x0: start,
                            x1: pen,
                            y: run.baseline + (asc + uthick) * run.scale,
                            thickness: uthick * run.scale,
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
        width: width.max(pen),
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
            // Glyph origin sits at pen x = g.x, baseline shifted by g.y, scaled.
            x0 = x0.min(g.x + gx0 * g.scale);
            x1 = x1.max(g.x + gx1 * g.scale);
            y0 = y0.min(g.y + gy0 * g.scale);
            y1 = y1.max(g.y + gy1 * g.scale);
            found = true;
        }
    }
    found.then_some((x0, y0, x1, y1))
}
