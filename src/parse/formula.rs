//! Tiny arithmetic-expression evaluator for axis tick-label formulas
//! (`ticklabel formula "$t-273.15"`).
//!
//! Grace evaluates the formula with its full command-language interpreter,
//! `$t` bound to the vector of major tick positions (drawticks.cpp
//! `calculate_tickgrid`). Real project files use plain arithmetic on `$t`,
//! so this evaluator supports exactly that subset: numbers, `$t`, the four
//! operators, `^` for powers, unary minus and parentheses.

/// Evaluate `expr` with `$t = t`. Returns `None` on any parse error, in
/// which case the caller falls back to the untransformed value.
pub fn eval(expr: &str, t: f64) -> Option<f64> {
    let mut p = Parser {
        s: expr.as_bytes(),
        pos: 0,
        t,
    };
    let v = p.expr()?;
    p.skip_ws();
    if p.pos == p.s.len() {
        Some(v)
    } else {
        None
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    t: f64,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.s.get(self.pos).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // expr := term (('+'|'-') term)*
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat(b'+') {
                v += self.term()?;
            } else if self.eat(b'-') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
        }
    }

    // term := power (('*'|'/') power)*
    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        loop {
            if self.eat(b'*') {
                v *= self.power()?;
            } else if self.eat(b'/') {
                v /= self.power()?;
            } else {
                return Some(v);
            }
        }
    }

    // power := '-' power | atom ('^' power)?  — unary minus binds looser
    // than '^' (so "-$t^2" is -($t^2)), and '^' is right-associative.
    fn power(&mut self) -> Option<f64> {
        if self.eat(b'-') {
            return Some(-self.power()?);
        }
        let v = self.atom()?;
        if self.eat(b'^') {
            let e = self.power()?;
            Some(v.powf(e))
        } else {
            Some(v)
        }
    }

    // atom := '(' expr ')' | '$t' | number
    fn atom(&mut self) -> Option<f64> {
        match self.peek()? {
            b'(' => {
                self.pos += 1;
                let v = self.expr()?;
                if self.eat(b')') {
                    Some(v)
                } else {
                    None
                }
            }
            b'$' => {
                self.pos += 1;
                if self.s.get(self.pos).map(|c| c.to_ascii_lowercase()) == Some(b't') {
                    self.pos += 1;
                    Some(self.t)
                } else {
                    None
                }
            }
            c if c.is_ascii_digit() || c == b'.' => {
                let start = self.pos;
                while self
                    .s
                    .get(self.pos)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
                {
                    self.pos += 1;
                }
                // Exponent part (e.g. 1e-3).
                if self
                    .s
                    .get(self.pos)
                    .is_some_and(|c| *c == b'e' || *c == b'E')
                {
                    let mut p = self.pos + 1;
                    if self.s.get(p).is_some_and(|c| *c == b'+' || *c == b'-') {
                        p += 1;
                    }
                    if self.s.get(p).is_some_and(|c| c.is_ascii_digit()) {
                        self.pos = p;
                        while self.s.get(self.pos).is_some_and(|c| c.is_ascii_digit()) {
                            self.pos += 1;
                        }
                    }
                }
                std::str::from_utf8(&self.s[start..self.pos])
                    .ok()?
                    .parse()
                    .ok()
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::eval;

    #[test]
    fn formula_arithmetic() {
        assert_eq!(eval("$t-273.15", 473.15), Some(200.0));
        assert_eq!(eval("2*$t + 1", 3.0), Some(7.0));
        assert_eq!(eval("($t+1)/2", 3.0), Some(2.0));
        assert_eq!(eval("-$t^2", 3.0), Some(-9.0));
        assert_eq!(eval("1e-3*$t", 2000.0), Some(2.0));
        assert_eq!(eval("nonsense", 1.0), None);
    }
}
