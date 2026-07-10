//! LaTeX-subset → Grace-markup transpiler: paired `$...$` math regions in
//! any text string are rewritten into the native markup `text.rs` already
//! renders, so LaTeX input needs no new layout engine, fonts or
//! dependencies — both render backends, hit-testing and `text::plain`
//! (GUI labels) pick it up unchanged.
//!
//! Coverage: Greek letters and the Adobe-Symbol operator set, nested
//! super/subscripts (a combined `x_i^2` stacks both via pen marks),
//! `\sqrt` (with optional index), `\bar`/`\overline`/`\underline`, styles
//! (`\mathrm`, `\mathbf`, `\mathit`, `\text`…), upright function names
//! (`\sin` …), spacing (`\,` `\:` `\;` `\!` `\quad` `\qquad`) and a
//! textual `\frac{a}{b}` (rendered `a/b`). Variables render italic,
//! digits and punctuation in the label's own font, per math convention.
//!
//! Strictness keeps existing files byte-identical: a string without a
//! paired, non-empty `$...$` region passes through unchanged
//! (`Cow::Borrowed`), and a region that fails to parse (unknown command,
//! stray brace) is emitted literally, dollars included. `\$` escapes a
//! literal dollar.

use std::borrow::Cow;

/// Expand `$...$` math regions into Grace markup (see module docs).
pub fn expand(input: &str) -> Cow<'_, str> {
    if !input.contains('$') {
        return Cow::Borrowed(input);
    }
    let chars: Vec<char> = input.chars().collect();
    // Scan for unescaped dollars, skipping backslash escapes so `\$` (and
    // a `\\` before a real delimiter) can't confuse the pairing.
    let mut delims = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '$' => {
                delims.push(i);
                i += 1;
            }
            _ => i += 1,
        }
    }
    if delims.is_empty() || delims.len() % 2 != 0 {
        return Cow::Borrowed(input); // no regions / unpaired: untouched
    }

    let mut out = String::with_capacity(input.len() + 32);
    let mut pos = 0;
    for pair in delims.chunks_exact(2) {
        text_verbatim(&mut out, &chars[pos..pair[0]]);
        let content = &chars[pair[0] + 1..pair[1]];
        match math_region(content) {
            Some(m) => out.push_str(&m),
            None => {
                // Unparseable region: emit literally, dollars included.
                out.push('$');
                out.extend(content.iter());
                out.push('$');
            }
        }
        pos = pair[1] + 1;
    }
    text_verbatim(&mut out, &chars[pos..]);
    Cow::Owned(out)
}

/// Copy a non-math span, resolving `\$` to a literal dollar.
fn text_verbatim(out: &mut String, chars: &[char]) {
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'$') {
            out.push('$');
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
}

// ---------------------------------------------------------------- parsing

/// One math atom: a nucleus with optional sub/superscript.
struct Atom {
    node: Node,
    sup: Option<Vec<Atom>>,
    sub: Option<Vec<Atom>>,
}

impl Atom {
    fn plain(node: Node) -> Self {
        Atom { node, sup: None, sub: None }
    }
}

enum Node {
    /// Literal character (font decided by class: letters italic).
    Ch(char),
    /// A character in the Adobe-Symbol-encoded Symbol font.
    Sym(char),
    Group(Vec<Atom>),
    /// Forced font for the whole argument.
    Styled(EFont, Vec<Atom>),
    /// Upright function name (`\sin` …).
    Func(&'static str),
    /// Horizontal space in em of the current size.
    Space(f32),
    Sqrt { index: Option<Vec<Atom>>, arg: Vec<Atom> },
    Over(Vec<Atom>),
    Under(Vec<Atom>),
    /// Textual fraction, rendered `a/b`.
    Frac(Vec<Atom>, Vec<Atom>),
}

struct Parser {
    s: Vec<char>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    /// Parse a sequence of atoms up to `}` (not consumed) or end of input.
    fn atoms(&mut self, depth: u8) -> Result<Vec<Atom>, ()> {
        if depth > 24 {
            return Err(());
        }
        let mut out: Vec<Atom> = Vec::new();
        loop {
            match self.peek() {
                None | Some('}') => return Ok(out),
                Some(c @ ('^' | '_')) => {
                    self.i += 1;
                    let arg = self.arg(depth + 1)?;
                    if out.is_empty() {
                        // `{}^2`-style empty base.
                        out.push(Atom::plain(Node::Group(Vec::new())));
                    }
                    let atom = out.last_mut().unwrap();
                    let slot = if c == '^' { &mut atom.sup } else { &mut atom.sub };
                    if slot.is_some() {
                        return Err(()); // double superscript
                    }
                    *slot = Some(arg);
                }
                Some(c) if c.is_whitespace() => self.i += 1, // math ignores spaces
                Some('\\') => {
                    let node = self.command(depth + 1)?;
                    out.push(Atom::plain(node));
                }
                Some('{') => {
                    self.i += 1;
                    let inner = self.atoms(depth + 1)?;
                    if self.peek() != Some('}') {
                        return Err(());
                    }
                    self.i += 1;
                    out.push(Atom::plain(Node::Group(inner)));
                }
                Some(c) => {
                    self.i += 1;
                    out.push(Atom::plain(Node::Ch(c)));
                }
            }
        }
    }

    /// A single argument: `{...}`, a command, or one character.
    fn arg(&mut self, depth: u8) -> Result<Vec<Atom>, ()> {
        while self.peek().is_some_and(|c| c.is_whitespace()) {
            self.i += 1;
        }
        match self.peek() {
            Some('{') => {
                self.i += 1;
                let inner = self.atoms(depth)?;
                if self.peek() != Some('}') {
                    return Err(());
                }
                self.i += 1;
                Ok(inner)
            }
            Some('\\') => Ok(vec![Atom::plain(self.command(depth)?)]),
            Some(c) if c != '}' && c != '^' && c != '_' => {
                self.i += 1;
                Ok(vec![Atom::plain(Node::Ch(c))])
            }
            _ => Err(()),
        }
    }

    /// A `\command` (opening backslash not yet consumed).
    fn command(&mut self, depth: u8) -> Result<Node, ()> {
        self.i += 1; // the backslash
        let c = self.peek().ok_or(())?;
        // Single-character escapes and spacing shorthands.
        if !c.is_ascii_alphabetic() {
            self.i += 1;
            return match c {
                '{' | '}' | '$' | '%' | '&' | '#' | '_' | '^' | '|' => Ok(Node::Ch(c)),
                ',' => Ok(Node::Space(0.167)),
                ':' => Ok(Node::Space(0.222)),
                ';' => Ok(Node::Space(0.278)),
                '!' => Ok(Node::Space(-0.167)),
                _ => Err(()),
            };
        }
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.i += 1;
        }
        let name: String = self.s[start..self.i].iter().collect();

        if let Some(sym) = symbol_char(&name) {
            return Ok(Node::Sym(sym));
        }
        if let Some(func) = function_name(&name) {
            return Ok(Node::Func(func));
        }
        match name.as_str() {
            "sqrt" => {
                let index = if self.peek() == Some('[') {
                    self.i += 1;
                    let idx = self.atoms(depth)?;
                    if self.peek() != Some(']') {
                        return Err(());
                    }
                    self.i += 1;
                    Some(idx)
                } else {
                    None
                };
                Ok(Node::Sqrt { index, arg: self.arg(depth)? })
            }
            "frac" => Ok(Node::Frac(self.arg(depth)?, self.arg(depth)?)),
            "bar" | "overline" => Ok(Node::Over(self.arg(depth)?)),
            "underline" => Ok(Node::Under(self.arg(depth)?)),
            "mathrm" | "textrm" | "operatorname" => Ok(Node::Styled(EFont::Roman, self.arg(depth)?)),
            "mathbf" | "textbf" | "boldsymbol" | "bm" => Ok(Node::Styled(EFont::Bold, self.arg(depth)?)),
            "mathit" | "textit" | "mathnormal" => Ok(Node::Styled(EFont::Italic, self.arg(depth)?)),
            "text" => Ok(Node::Styled(EFont::Base, self.arg(depth)?)),
            "quad" => Ok(Node::Space(1.0)),
            "qquad" => Ok(Node::Space(2.0)),
            // Delimiter sizing is not supported; keep the delimiter itself
            // (`\left.` / `\right.` keep nothing).
            "left" | "right" => match self.peek() {
                Some('.') => {
                    self.i += 1;
                    Ok(Node::Group(Vec::new()))
                }
                Some('\\') => self.command(depth),
                Some(d) => {
                    self.i += 1;
                    Ok(Node::Ch(d))
                }
                None => Err(()),
            },
            _ => Err(()),
        }
    }
}

// --------------------------------------------------------------- emitting

/// Fonts the emitter switches between. `Base` is the label's own font.
#[derive(Clone, Copy, PartialEq)]
enum EFont {
    Base,
    Roman,
    Italic,
    Bold,
    Symbol,
}

/// Emits Grace markup while mirroring the engine's size/shift state, so
/// nested scripts compose exactly and pen marks can restore the baseline.
struct Emit {
    out: String,
    /// Current absolute size factor (engine `tm.size()`), mirrored.
    size: f32,
    /// Current absolute vertical shift in base-em units, mirrored.
    v: f32,
    font: EFont,
    /// Next pen-mark id (starts high to avoid colliding with user marks).
    mark: i32,
}

/// `\s`/`\S` engine constants (`t1fonts.h`), reused so LaTeX scripts match
/// native Grace ones; entry/exit pairs are exact textual inverses.
const SUP_IN: &str = "\\v{0.6}\\z{0.707107}";
const SUP_OUT: &str = "\\z{1.414214}\\v{-0.6}";
const SUB_IN: &str = "\\v{-0.4}\\z{0.707107}";
const SUB_OUT: &str = "\\z{1.414214}\\v{0.4}";

impl Emit {
    fn set_font(&mut self, f: EFont) {
        if self.font == f {
            return;
        }
        self.out.push_str(match f {
            EFont::Base => "\\f{}",
            EFont::Roman => "\\f{Times-Roman}",
            EFont::Italic => "\\f{Times-Italic}",
            EFont::Bold => "\\f{Times-Bold}",
            EFont::Symbol => "\\x",
        });
        self.font = f;
    }

    /// One character in the given font, escaping markup-active chars.
    fn ch(&mut self, c: char, f: EFont) {
        self.set_font(f);
        if c == '\\' {
            self.out.push_str("\\\\");
        } else {
            self.out.push(c);
        }
    }

    fn enter_sup(&mut self) {
        self.out.push_str(SUP_IN);
        self.v += 0.6 * self.size;
        self.size *= std::f32::consts::FRAC_1_SQRT_2;
    }

    fn exit_sup(&mut self) {
        self.out.push_str(SUP_OUT);
        self.size /= std::f32::consts::FRAC_1_SQRT_2;
        self.v -= 0.6 * self.size;
    }

    fn enter_sub(&mut self) {
        self.out.push_str(SUB_IN);
        self.v -= 0.4 * self.size;
        self.size *= std::f32::consts::FRAC_1_SQRT_2;
    }

    fn exit_sub(&mut self) {
        self.out.push_str(SUB_OUT);
        self.size /= std::f32::consts::FRAC_1_SQRT_2;
        self.v += 0.4 * self.size;
    }

    fn atoms(&mut self, list: &[Atom], forced: Option<EFont>) {
        for a in list {
            self.atom(a, forced);
        }
    }

    fn atom(&mut self, a: &Atom, forced: Option<EFont>) {
        self.node(&a.node, forced);
        match (&a.sub, &a.sup) {
            (Some(sb), Some(sp)) => {
                // Both scripts stack at the same x: mark the pen after the
                // nucleus, draw the subscript, return, draw the superscript.
                let id = self.mark;
                self.mark += 1;
                let v_ctx = self.v;
                self.out.push_str(&format!("\\m{{{id}}}"));
                self.enter_sub();
                self.atoms(sb, forced);
                self.exit_sub();
                self.out.push_str(&format!("\\M{{{id}}}"));
                // `\M` also resets the engine's vshift to the baseline.
                self.v = 0.0;
                if v_ctx.abs() > 1e-6 {
                    self.out.push_str(&format!("\\v{{{:.6}}}", v_ctx / self.size));
                    self.v = v_ctx;
                }
                self.enter_sup();
                self.atoms(sp, forced);
                self.exit_sup();
            }
            (Some(sb), None) => {
                self.enter_sub();
                self.atoms(sb, forced);
                self.exit_sub();
            }
            (None, Some(sp)) => {
                self.enter_sup();
                self.atoms(sp, forced);
                self.exit_sup();
            }
            (None, None) => {}
        }
    }

    fn node(&mut self, n: &Node, forced: Option<EFont>) {
        match n {
            Node::Ch(c) => {
                let f = forced.unwrap_or(if c.is_ascii_alphabetic() {
                    EFont::Italic // math variables are italic
                } else {
                    EFont::Base
                });
                self.ch(*c, f);
            }
            Node::Sym(c) => self.ch(*c, EFont::Symbol),
            Node::Group(l) => self.atoms(l, forced),
            Node::Styled(f, l) => self.atoms(l, Some(*f)),
            Node::Func(name) => {
                for c in name.chars() {
                    self.ch(c, EFont::Roman);
                }
            }
            Node::Space(x) => self.out.push_str(&format!("\\h{{{x}}}")),
            Node::Sqrt { index, arg } => {
                if let Some(idx) = index {
                    self.enter_sup();
                    self.atoms(idx, forced);
                    self.exit_sup();
                }
                // Radical sign + overlined radicand (the classic Grace
                // idiom; 0xD6 is the Adobe-Symbol radical).
                self.ch('\u{D6}', EFont::Symbol);
                self.out.push_str("\\o");
                self.atoms(arg, forced);
                self.out.push_str("\\O");
            }
            Node::Over(l) => {
                self.out.push_str("\\o");
                self.atoms(l, forced);
                self.out.push_str("\\O");
            }
            Node::Under(l) => {
                self.out.push_str("\\u");
                self.atoms(l, forced);
                self.out.push_str("\\U");
            }
            Node::Frac(num, den) => {
                self.operand(num, forced);
                self.ch('/', EFont::Base);
                self.operand(den, forced);
            }
        }
    }

    /// A fraction operand, parenthesized unless it is a single simple atom.
    fn operand(&mut self, l: &[Atom], forced: Option<EFont>) {
        let simple = l.len() == 1
            && l[0].sup.is_none()
            && l[0].sub.is_none()
            && matches!(l[0].node, Node::Ch(_) | Node::Sym(_) | Node::Func(_));
        if simple {
            self.atoms(l, forced);
        } else {
            self.ch('(', EFont::Base);
            self.atoms(l, forced);
            self.ch(')', EFont::Base);
        }
    }
}

/// Transpile one math region; `None` = fall back to literal text.
fn math_region(content: &[char]) -> Option<String> {
    let mut p = Parser { s: content.to_vec(), i: 0 };
    let atoms = p.atoms(0).ok()?;
    if p.i != p.s.len() || atoms.is_empty() {
        return None; // stray `}` / empty region
    }
    let mut e = Emit { out: String::new(), size: 1.0, v: 0.0, font: EFont::Base, mark: 100 };
    e.atoms(&atoms, None);
    e.set_font(EFont::Base); // restore the label's own font
    Some(e.out)
}

/// Greek letters and operators as Adobe-Symbol-encoded characters (the
/// codes the bundled StandardSymbolsPS maps natively; `text.rs` sends them
/// through unchanged and `font::symbol_to_unicode` shows them in labels).
fn symbol_char(name: &str) -> Option<char> {
    Some(match name {
        // Greek, lowercase.
        "alpha" => 'a',
        "beta" => 'b',
        "gamma" => 'g',
        "delta" => 'd',
        "epsilon" | "varepsilon" => 'e',
        "zeta" => 'z',
        "eta" => 'h',
        "theta" => 'q',
        "vartheta" => 'J',
        "iota" => 'i',
        "kappa" => 'k',
        "lambda" => 'l',
        "mu" => 'm',
        "nu" => 'n',
        "xi" => 'x',
        "omicron" => 'o',
        "pi" => 'p',
        "varpi" => 'v',
        "rho" => 'r',
        "sigma" => 's',
        "varsigma" => 'V',
        "tau" => 't',
        "upsilon" => 'u',
        "phi" => 'f',
        "varphi" => 'j',
        "chi" => 'c',
        "psi" => 'y',
        "omega" => 'w',
        // Greek, uppercase.
        "Gamma" => 'G',
        "Delta" => 'D',
        "Theta" => 'Q',
        "Lambda" => 'L',
        "Xi" => 'X',
        "Pi" => 'P',
        "Sigma" => 'S',
        "Upsilon" => 'U',
        "Phi" => 'F',
        "Psi" => 'Y',
        "Omega" => 'W',
        // Operators and relations (Adobe Symbol high codes).
        "pm" => '\u{B1}',
        "times" => '\u{B4}',
        "div" => '\u{B8}',
        "cdot" => '\u{D7}',
        "ast" => '*',
        "leq" | "le" => '\u{A3}',
        "geq" | "ge" => '\u{B3}',
        "neq" | "ne" => '\u{B9}',
        "approx" => '\u{BB}',
        "equiv" => '\u{BA}',
        "sim" => '\u{7E}',
        "propto" => '\u{B5}',
        "infty" => '\u{A5}',
        "partial" => '\u{B6}',
        "nabla" => '\u{D1}',
        "sum" => '\u{E5}',
        "prod" => '\u{D5}',
        "int" => '\u{F2}',
        "in" => '\u{CE}',
        "notin" => '\u{CF}',
        "ni" => '\u{27}',
        "cup" => '\u{C8}',
        "cap" => '\u{C7}',
        "subset" => '\u{CC}',
        "supset" => '\u{C9}',
        "subseteq" => '\u{CD}',
        "supseteq" => '\u{CA}',
        "emptyset" | "varnothing" => '\u{C6}',
        "forall" => '\u{22}',
        "exists" => '\u{24}',
        "neg" | "lnot" => '\u{D8}',
        "wedge" | "land" => '\u{D9}',
        "vee" | "lor" => '\u{DA}',
        "leftarrow" => '\u{AC}',
        "uparrow" => '\u{AD}',
        "rightarrow" | "to" => '\u{AE}',
        "downarrow" => '\u{AF}',
        "leftrightarrow" => '\u{AB}',
        "Leftarrow" => '\u{DC}',
        "Uparrow" => '\u{DD}',
        "Rightarrow" => '\u{DE}',
        "Downarrow" => '\u{DF}',
        "Leftrightarrow" => '\u{DB}',
        "circ" | "degree" => '\u{B0}',
        "prime" => '\u{A2}',
        "bullet" => '\u{B7}',
        "ldots" | "cdots" | "dots" => '\u{BC}',
        "aleph" => '\u{C0}',
        "Im" => '\u{C1}',
        "Re" => '\u{C2}',
        "wp" => '\u{C3}',
        "otimes" => '\u{C4}',
        "oplus" => '\u{C5}',
        "angle" => '\u{D0}',
        "surd" => '\u{D6}',
        "langle" => '\u{E1}',
        "rangle" => '\u{F1}',
        "therefore" => '\\',
        _ => return None,
    })
}

/// Function names rendered upright, per math convention.
fn function_name(name: &str) -> Option<&'static str> {
    const FUNCS: [&str; 24] = [
        "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh",
        "tanh", "coth", "exp", "log", "ln", "lg", "det", "dim", "lim", "min", "max", "arg", "deg",
    ];
    FUNCS.iter().find(|f| **f == name).copied()
}

#[cfg(test)]
mod tests {
    use super::expand;
    use std::borrow::Cow;

    /// Strings without a paired math region pass through untouched — the
    /// whole `.agr` corpus must stay byte-identical.
    #[test]
    fn passthrough_is_byte_identical() {
        for s in [
            "no dollars at all",
            "unpaired $ dollar",
            r" ! #$%&'()*+,-./0123456789:;<=>?@ABC", // tfonts charset string
            "$t/PI",                                 // ticklabel formula
            r"escaped \$5 only",
        ] {
            assert!(matches!(expand(s), Cow::Borrowed(_)), "{s:?} must not change");
        }
        // An unparseable region falls back to its literal text.
        assert_eq!(expand(r"$\unknowncmd x$"), r"$\unknowncmd x$");
        assert_eq!(expand("$ $"), "$ $");
    }

    #[test]
    fn greek_and_symbols() {
        assert_eq!(expand(r"$\alpha$"), "\\xa\\f{}");
        assert_eq!(expand(r"$\pm$"), "\\x\u{B1}\\f{}");
        // Adjacent Symbol chars share one font switch.
        assert_eq!(expand(r"$\alpha\beta$"), "\\xab\\f{}");
    }

    #[test]
    fn scripts_nest_and_stack() {
        // Simple superscript: italic base, script entry/exit are inverses.
        assert_eq!(
            expand("$x^2$"),
            "\\f{Times-Italic}x\\v{0.6}\\z{0.707107}\\f{}2\\z{1.414214}\\v{-0.6}"
        );
        // Combined sub+sup stack via pen marks.
        let both = expand("$x_i^2$");
        assert!(both.contains("\\m{100}") && both.contains("\\M{100}"), "{both}");
        // Nesting parses (exact string is covered by render tests).
        assert!(matches!(expand("$x^{y^2}$"), Cow::Owned(_)));
    }

    #[test]
    fn commands_and_text() {
        let sqrt = expand(r"$\sqrt{x}$");
        assert!(sqrt.contains('\u{D6}') && sqrt.contains("\\o"), "{sqrt}");
        assert_eq!(expand(r"$\frac{a}{b+1}$"), {
            // numerator simple, denominator parenthesized
            "\\f{Times-Italic}a\\f{}/(\\f{Times-Italic}b\\f{}+1)".to_string()
        });
        assert_eq!(expand(r"$\mathrm{d}x$"), "\\f{Times-Roman}d\\f{Times-Italic}x\\f{}");
        // Text outside regions is kept, `\$` unescapes when regions exist.
        assert_eq!(expand(r"cost \$5, angle $\theta$"), "cost $5, angle \\xq\\f{}");
    }
}
