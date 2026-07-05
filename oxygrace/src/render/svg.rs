//! SVG output backend.
//!
//! Receives the same device-space paths as the raster backend, so the SVG
//! matches the PNG rendering exactly. Text arrives as glyph outline paths
//! (faithful to Grace's typesetting and independent of viewer fonts, at the
//! cost of non-selectable text). Hatch fills become `<pattern>` definitions
//! with the same 16x16 fg-on-bg tiles; clipping becomes `<clipPath>`
//! references on the affected elements.

use std::collections::HashMap;
use std::fmt::Write;

use tiny_skia::{FillRule, Path, PathSegment};

use crate::color::Rgba;
use crate::patterns::PATTERN_BITS;

use super::canvas::FillPaint;

pub(crate) struct SvgBackend {
    width: u32,
    height: u32,
    /// Drawn elements, in order.
    body: String,
    /// `<defs>` content: clip paths and pattern tiles.
    defs: String,
    /// ` clip-path="url(#cN)"` while a clip is active, else empty.
    clip_attr: String,
    /// Deduplicated clip rectangles, keyed by their coordinate bits.
    clip_ids: HashMap<(u32, u32, u32, u32), usize>,
    /// Deduplicated pattern tiles, keyed by (pattern, fg, bg).
    pattern_ids: HashMap<(i32, [u8; 4], [u8; 4]), usize>,
}

/// Format a coordinate compactly (two decimals, trailing zeros trimmed).
fn fmt(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// `#rrggbb` for a resolved color.
fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// ` <attr>-opacity="…"` for a translucent color, empty when opaque (pen
/// alpha, QtGrace `ALPHA_CHANNELS`).
fn opacity_attr(attr: &str, c: Rgba) -> String {
    if c.a == 255 {
        String::new()
    } else {
        format!(" {}-opacity=\"{}\"", attr, fmt(c.a as f32 / 255.0))
    }
}

/// Serialize a tiny-skia path into SVG path data.
fn path_data(path: &Path) -> String {
    let mut d = String::new();
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                let _ = write!(d, "M{} {}", fmt(p.x), fmt(p.y));
            }
            PathSegment::LineTo(p) => {
                let _ = write!(d, "L{} {}", fmt(p.x), fmt(p.y));
            }
            PathSegment::QuadTo(c, p) => {
                let _ = write!(d, "Q{} {} {} {}", fmt(c.x), fmt(c.y), fmt(p.x), fmt(p.y));
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {}",
                    fmt(c1.x),
                    fmt(c1.y),
                    fmt(c2.x),
                    fmt(c2.y),
                    fmt(p.x),
                    fmt(p.y)
                );
            }
            PathSegment::Close => d.push('Z'),
        }
    }
    d
}

impl SvgBackend {
    pub(crate) fn new(width: u32, height: u32) -> Self {
        SvgBackend {
            width,
            height,
            // The page starts white, like the raster canvas.
            body: format!(
                "<rect width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>\n",
                width, height
            ),
            defs: String::new(),
            clip_attr: String::new(),
            clip_ids: HashMap::new(),
            pattern_ids: HashMap::new(),
        }
    }

    pub(crate) fn set_clip(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let key = (x0.to_bits(), y0.to_bits(), x1.to_bits(), y1.to_bits());
        let next = self.clip_ids.len();
        let defs = &mut self.defs;
        let id = *self.clip_ids.entry(key).or_insert_with(|| {
            let _ = writeln!(
                defs,
                "<clipPath id=\"c{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                next,
                fmt(x0),
                fmt(y0),
                fmt(x1 - x0),
                fmt(y1 - y0)
            );
            next
        });
        self.clip_attr = format!(" clip-path=\"url(#c{})\"", id);
    }

    pub(crate) fn clear_clip(&mut self) {
        self.clip_attr.clear();
    }

    pub(crate) fn stroke_path(
        &mut self,
        path: &Path,
        color: Rgba,
        width: f32,
        dash: Option<&[f32]>,
    ) {
        let mut attrs = format!(
            " fill=\"none\" stroke=\"{}\"{} stroke-width=\"{}\"",
            hex(color),
            opacity_attr("stroke", color),
            fmt(width)
        );
        if let Some(dash) = dash {
            let list: Vec<String> = dash.iter().map(|&d| fmt(d)).collect();
            let _ = write!(attrs, " stroke-dasharray=\"{}\"", list.join(","));
        }
        let _ = writeln!(
            self.body,
            "<path d=\"{}\"{}{}/>",
            path_data(path),
            attrs,
            self.clip_attr
        );
    }

    pub(crate) fn fill_path(&mut self, path: &Path, fill: &FillPaint, rule: FillRule) {
        let (fill_attr, opacity) = match fill {
            FillPaint::Solid(c) => (hex(*c), opacity_attr("fill", *c)),
            // Pattern tiles carry their own per-color opacity in the defs.
            FillPaint::Hatch { pattern, fg, bg } => {
                (format!("url(#p{})", self.pattern_id(*pattern, *fg, *bg)), String::new())
            }
        };
        let rule_attr = match rule {
            FillRule::EvenOdd => " fill-rule=\"evenodd\"",
            FillRule::Winding => "",
        };
        let _ = writeln!(
            self.body,
            "<path d=\"{}\" fill=\"{}\"{}{}{}/>",
            path_data(path),
            fill_attr,
            opacity,
            rule_attr,
            self.clip_attr
        );
    }

    /// Define (once) the 16x16 tile for a hatch pattern: an opaque background
    /// rectangle with the set bits drawn in the foreground color, exactly
    /// like the raster tile.
    fn pattern_id(&mut self, pattern: i32, fg: Rgba, bg: Rgba) -> usize {
        let key = (pattern, [fg.r, fg.g, fg.b, fg.a], [bg.r, bg.g, bg.b, bg.a]);
        let next = self.pattern_ids.len();
        let defs = &mut self.defs;
        *self.pattern_ids.entry(key).or_insert_with(|| {
            let bits = &PATTERN_BITS[pattern as usize];
            let mut cells = String::new();
            for row in 0..16usize {
                for col in 0..16usize {
                    // LSB-first within each byte (X11 bitmap order).
                    let byte = bits[row * 2 + col / 8];
                    if (byte >> (col % 8)) & 1 == 1 {
                        let _ = write!(cells, "M{} {}h1v1h-1z", col, row);
                    }
                }
            }
            let _ = writeln!(
                defs,
                "<pattern id=\"p{}\" width=\"16\" height=\"16\" patternUnits=\"userSpaceOnUse\">\
                 <rect width=\"16\" height=\"16\" fill=\"{}\"{}/><path d=\"{}\" fill=\"{}\"{}/></pattern>",
                next,
                hex(bg),
                opacity_attr("fill", bg),
                cells,
                hex(fg),
                opacity_attr("fill", fg)
            );
            next
        })
    }

    /// Assemble the final document.
    pub(crate) fn finish(self) -> String {
        let mut out = String::with_capacity(self.body.len() + self.defs.len() + 256);
        let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        let _ = writeln!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">",
            w = self.width,
            h = self.height
        );
        if !self.defs.is_empty() {
            let _ = writeln!(out, "<defs>\n{}</defs>", self.defs);
        }
        out.push_str(&self.body);
        out.push_str("</svg>\n");
        out
    }
}
