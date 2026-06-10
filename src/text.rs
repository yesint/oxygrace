//! Grace string markup parsing and layout.
//!
//! Grace strings embed formatting escapes introduced by `\`. This module turns
//! a marked-up UTF-8 string into a sequence of [`StyledRun`]s (runs of plain
//! text sharing a font, scale, baseline shift and optional color), then lays
//! them out into positioned glyphs.
//!
//! Milestone 1 supports the common escapes: `\f{n}` / `\digit` (font),
//! `\x` (symbol font), `\s` / `\S` / `\N` (sub/superscript/normal),
//! `\+` / `\-` (enlarge/shrink), `\R{n}` (color) and `\\`. Unknown escapes are
//! dropped. This keeps plain text and the typical sub/superscripts correct
//! while ignoring the rarer typographic controls.

use crate::font::FontSet;

const SSCRIPT_SCALE: f32 = 0.6;
const SSCRIPT_SHIFT: f32 = 0.4;
const ENLARGE: f32 = 1.19;

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
}

/// Parse Grace markup into styled runs, starting from a base font.
pub fn parse(input: &str, base_font: i32) -> Vec<StyledRun> {
    let mut runs: Vec<StyledRun> = Vec::new();
    let mut font = base_font;
    let mut scale = 1.0f32;
    let mut baseline = 0.0f32;
    let mut color: Option<i32> = None;
    let mut buf = String::new();

    // Flush the current buffer into a run with the current style.
    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                runs.push(StyledRun {
                    text: std::mem::take(&mut buf),
                    font,
                    scale,
                    baseline,
                    color,
                });
            }
        };
    }

    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            buf.push(c);
            continue;
        }
        // Escape sequence.
        let Some(&next) = chars.peek() else { break };
        match next {
            '\\' => {
                buf.push('\\');
                chars.next();
            }
            // Font selection by single digit: \0..\9
            '0'..='9' => {
                flush!();
                font = next.to_digit(10).unwrap() as i32;
                chars.next();
            }
            // Parameterized escapes \f{..}, \R{..}, etc.
            'f' | 'R' | 'Z' | 'z' => {
                let kind = next;
                chars.next();
                if chars.peek() == Some(&'{') {
                    chars.next();
                    let mut arg = String::new();
                    for ac in chars.by_ref() {
                        if ac == '}' {
                            break;
                        }
                        arg.push(ac);
                    }
                    match kind {
                        'f' => {
                            flush!();
                            font = parse_font_arg(&arg, base_font);
                        }
                        'R' => {
                            flush!();
                            color = arg.trim().parse::<i32>().ok();
                        }
                        'z' => {
                            flush!();
                            if let Ok(v) = arg.trim().parse::<f32>() {
                                scale = v;
                            }
                        }
                        _ => {}
                    }
                }
            }
            'x' => {
                flush!();
                font = 12; // Symbol font
                chars.next();
            }
            's' => {
                flush!();
                scale = SSCRIPT_SCALE;
                baseline = -SSCRIPT_SHIFT;
                chars.next();
            }
            'S' => {
                flush!();
                scale = SSCRIPT_SCALE;
                baseline = SSCRIPT_SHIFT;
                chars.next();
            }
            'N' => {
                flush!();
                scale = 1.0;
                baseline = 0.0;
                chars.next();
            }
            '+' => {
                flush!();
                scale *= ENLARGE;
                chars.next();
            }
            '-' => {
                flush!();
                scale /= ENLARGE;
                chars.next();
            }
            // Line break: Grace `\n` drops the baseline by exactly one em and
            // returns the pen to the start x (t1fonts.cpp, case 'n':
            // `baseline -= 1.0`, goto MARK_CR). Encoded as a literal newline
            // in the run text and handled in `layout`.
            'n' => {
                buf.push('\n');
                chars.next();
            }
            // Reset to defaults.
            'B' => {
                flush!();
                font = base_font;
                scale = 1.0;
                baseline = 0.0;
                color = None;
                chars.next();
            }
            // Unknown single-char escape: drop the marker char.
            _ => {
                chars.next();
            }
        }
    }
    flush!();
    runs
}

fn parse_font_arg(arg: &str, base_font: i32) -> i32 {
    let a = arg.trim();
    if let Ok(n) = a.parse::<i32>() {
        return n;
    }
    // Allow a few common font names.
    match a.to_ascii_lowercase().as_str() {
        "times-roman" | "times" => 0,
        "helvetica" => 4,
        "courier" => 8,
        "symbol" => 12,
        _ => base_font,
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

/// Result of laying out a marked-up string: positioned glyphs plus total width.
pub struct Layout {
    pub glyphs: Vec<LaidGlyph>,
    /// Total advance width in em units of the base size.
    pub width: f32,
}

/// Lay out a marked-up string into positioned glyphs (em units, base size 1).
pub fn layout(fonts: &FontSet, input: &str, base_font: i32) -> Layout {
    let runs = parse(input, base_font);
    let mut glyphs = Vec::new();
    let mut pen = 0.0f32;
    let mut width = 0.0f32;
    // Extra baseline drop accumulated by `\n` breaks (one em per line).
    let mut line = 0.0f32;
    for run in &runs {
        for ch in run.text.chars() {
            if ch == '\n' {
                width = width.max(pen);
                pen = 0.0;
                line -= 1.0;
                continue;
            }
            let g = fonts.outline_char(run.font, ch);
            glyphs.push(LaidGlyph {
                font: run.font,
                x: pen,
                y: run.baseline + line,
                scale: run.scale,
                ch,
                color: run.color,
            });
            pen += g.advance * run.scale;
        }
    }
    Layout {
        glyphs,
        width: width.max(pen),
    }
}

/// Quick measurement of a marked-up string's advance width in em units.
pub fn measure(fonts: &FontSet, input: &str, base_font: i32) -> f32 {
    layout(fonts, input, base_font).width
}

/// Rendered ink bounding box of a marked-up string, in em units of the base
/// size: `(x_min, y_min, x_max, y_max)`, baseline-left origin, Y up. This is
/// the union of the positioned glyph outlines (mirrors Grace's `update_bbox`),
/// so it accounts for actual glyph extents, ascenders/descenders, and the
/// baseline shifts and scaling from sub/superscript markup. `None` if no glyph
/// has an outline (e.g. all spaces or an empty string).
pub fn bbox(fonts: &FontSet, input: &str, base_font: i32) -> Option<(f32, f32, f32, f32)> {
    let layout = layout(fonts, input, base_font);
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
