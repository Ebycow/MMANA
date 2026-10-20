//! Port of MMANA's numeric-field evaluator (`Calc` / `CalcI` / `CalcU`).
//!
//! It understands `+ - * /`, parentheses and the unit words `R` (wavelength),
//! `I` (inch) and `F` (foot). The precedence handling is reproduced exactly,
//! quirks included, so existing `.maa` files evaluate to the same numbers.

/// Context for the unit words.
#[derive(Debug, Clone, Copy)]
pub struct ExprContext {
    /// Value of `R` (one wavelength at the design frequency, metres).
    pub wavelength: f64,
}

impl Default for ExprContext {
    fn default() -> Self {
        Self { wavelength: 1.0 }
    }
}

/// Evaluates `text`. Returns the value and whether every term parsed; like the
/// original, a bad term still yields a (partial) value.
pub fn eval(text: &str, ctx: &ExprContext) -> (f64, bool) {
    let bytes = to_halfwidth(text);
    let mut parser = Parser { s: &bytes, ok: true, ctx };
    let mut vs = Term { d: 0.0, pr: 0, cd: b'+' };
    parser.calc(&mut vs, 0);
    (vs.d, parser.ok)
}

/// `CalcI`: rounds half away from zero.
pub fn eval_int(text: &str, ctx: &ExprContext) -> (i32, bool) {
    let (d, ok) = eval(text, ctx);
    let v = if d >= 0.0 { (d + 0.5) as i32 } else { (d - 0.5) as i32 };
    (v, ok)
}

/// `CalcU`: rounds the absolute value.
pub fn eval_uint(text: &str, ctx: &ExprContext) -> (i32, bool) {
    let (d, ok) = eval(text, ctx);
    ((d.abs() + 0.5) as i32, ok)
}

/// `ZSCopy` semantics: full-width ASCII becomes half-width, a few Japanese
/// punctuation marks map to ASCII, any other non-ASCII character becomes a
/// space. The original copies at most 255 bytes.
fn to_halfwidth(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let c = match ch {
            '\u{FF10}'..='\u{FF19}' => (ch as u32 - 0xFF10 + b'0' as u32) as u8,
            '\u{FF21}'..='\u{FF3A}' => (ch as u32 - 0xFF21 + b'A' as u32) as u8,
            '\u{FF41}'..='\u{FF5A}' => (ch as u32 - 0xFF41 + b'a' as u32) as u8,
            '．' | '。' => b'.',
            '，' | '、' => b',',
            '＋' => b'+',
            '－' | 'ー' => b'-',
            '＊' => b'*',
            '／' => b'/',
            '（' => b'(',
            '）' => b')',
            '＃' => b'#',
            '＝' => b'=',
            c if c.is_ascii() => c as u8,
            _ => b' ',
        };
        if out.len() >= 255 {
            break;
        }
        out.push(c);
    }
    out
}

struct Term {
    pr: i32,
    cd: u8,
    d: f64,
}

struct Parser<'a> {
    s: &'a [u8],
    ok: bool,
    ctx: &'a ExprContext,
}

fn priority(c: u8) -> i32 {
    match c {
        0 => 0,
        b'+' | b'-' => 1,
        b'*' | b'/' => 2,
        _ => -1,
    }
}

fn is_op(c: u8) -> bool {
    c != 0 && priority(c) != -1
}

fn apply(vs: &mut Term, vw: &Term) {
    match vs.cd {
        b'+' => vs.d += vw.d,
        b'-' => vs.d -= vw.d,
        b'*' => vs.d *= vw.d,
        b'/' => {
            if vw.d != 0.0 {
                vs.d /= vw.d;
            } else {
                vs.d = f64::MAX;
            }
        }
        _ => {}
    }
}

impl Parser<'_> {
    fn at(&self, p: usize) -> u8 {
        self.s.get(p).copied().unwrap_or(0)
    }

    fn skip_space(&self, mut p: usize) -> usize {
        while matches!(self.at(p), b' ' | b'\t') {
            p += 1;
        }
        p
    }

    /// `_val`: reads one operand and the operator that follows it.
    fn value(&mut self, vs: &mut Term, mut p: usize) -> usize {
        vs.cd = 0;
        vs.pr = 0;
        let mut buf: Vec<u8> = Vec::new();
        while self.at(p) != 0 {
            let c = self.at(p);
            let prev = if p > 0 { self.at(p - 1) } else { 0 };
            if is_op(c) && !buf.is_empty() && prev != b'E' && prev != b'e' {
                vs.cd = c;
                vs.pr = priority(c);
                p += 1;
                break;
            } else if c == b')' {
                p += 1;
                break;
            } else if c != b' ' && c != b'\t' {
                buf.push(c);
            }
            p += 1;
        }
        vs.d = if buf.is_empty() {
            0.0
        } else if buf.eq_ignore_ascii_case(b"R") {
            self.ctx.wavelength
        } else if buf.eq_ignore_ascii_case(b"I") {
            0.0254
        } else if buf.eq_ignore_ascii_case(b"F") {
            0.0254 * 12.0
        } else if let Some(v) = parse_float_prefix(&buf) {
            v
        } else {
            self.ok = false;
            0.0
        };
        p
    }

    /// `_calc`
    fn calc(&mut self, vs: &mut Term, p: usize) -> usize {
        let mut p = self.skip_space(p);
        if self.at(p) == 0 {
            return p;
        }
        let mut vw = Term { d: 0.0, pr: 0, cd: b'+' };
        if self.at(p) == b'(' {
            p += 1;
            p = self.calc(&mut vw, p);
            let c = self.at(p);
            if c != 0 && is_op(c) {
                vw.cd = c;
                vw.pr = priority(c);
                p += 1;
            }
        } else {
            p = self.value(&mut vw, p);
        }
        if vw.pr == 0 {
            apply(vs, &vw);
            p = self.skip_space(p);
            let c = self.at(p);
            if is_op(c) {
                vs.cd = c;
                vs.pr = priority(c);
                p += 1;
            }
        } else if vs.pr >= vw.pr {
            apply(vs, &vw);
            vs.pr = vw.pr;
            vs.cd = vw.cd;
            p = self.calc(vs, p);
        } else {
            p = self.calc(&mut vw, p);
            apply(vs, &vw);
            vs.pr = vw.pr;
            vs.cd = vw.cd;
        }
        p
    }
}

/// `sscanf("%lf")`: parses the longest decimal floating-point prefix.
fn parse_float_prefix(s: &[u8]) -> Option<f64> {
    let mut i = 0;
    if matches!(s.first(), Some(b'+' | b'-')) {
        i = 1;
    }
    let digits_start = i;
    while s.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let mut mantissa_digits = i - digits_start;
    if s.get(i) == Some(&b'.') {
        i += 1;
        let frac_start = i;
        while s.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        mantissa_digits += i - frac_start;
    }
    if mantissa_digits == 0 {
        return None;
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(s.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp_start = j;
        while s.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        if j > exp_start {
            i = j;
        }
    }
    std::str::from_utf8(&s[..i]).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> f64 {
        eval(s, &ExprContext { wavelength: 20.0 }).0
    }

    #[test]
    fn numbers() {
        assert_eq!(ev("1.5"), 1.5);
        assert_eq!(ev(" -2.6805"), -2.6805);
        assert_eq!(ev("8.000e-04"), 8.0e-4);
        assert_eq!(ev("1E+2"), 100.0);
        assert_eq!(ev(""), 0.0);
    }

    #[test]
    fn arithmetic_and_units() {
        assert_eq!(ev("1+2*3"), 7.0);
        assert_eq!(ev("R/4"), 5.0);
        assert_eq!(ev("(1+2)*3"), 9.0);
        assert_eq!(ev("2*I"), 0.0508);
        assert_eq!(ev("１．５"), 1.5);
        assert_eq!(ev("1/0"), f64::MAX);
    }

    #[test]
    fn rounding_helpers() {
        let ctx = ExprContext::default();
        assert_eq!(eval_int("-2.5", &ctx).0, -3);
        assert_eq!(eval_uint("-2.5", &ctx).0, 3);
        assert!(!eval("abc", &ctx).1);
    }
}
