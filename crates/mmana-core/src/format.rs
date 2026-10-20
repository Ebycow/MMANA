//! Number formatting identical to MMANA's `StrDbl` / `StrDblE`, used when
//! writing `.maa` files so that output stays byte-compatible.

/// `StrDbl`: `%lf` with trailing zeros removed, keeping one digit after the point.
pub fn str_dbl(d: f64) -> String {
    let mut s = format!("{d:.6}");
    while s.ends_with('0') && !s[..s.len() - 1].ends_with('.') {
        s.pop();
    }
    s
}

/// `StrDblE`: like [`str_dbl`], but values below 1e-3 use `%.3le` (`8.000e-04`).
pub fn str_dbl_e(d: f64) -> String {
    if d == 0.0 || d.abs() >= 1e-3 {
        return str_dbl(d);
    }
    let s = format!("{d:.3e}");
    let (mantissa, exp) = s.split_once('e').expect("exponent form");
    let exp: i32 = exp.parse().expect("integer exponent");
    let sign = if exp < 0 { '-' } else { '+' };
    format!("{mantissa}e{sign}{:02}", exp.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_dbl_matches_mmana() {
        assert_eq!(str_dbl(14.05), "14.05");
        assert_eq!(str_dbl(2.0), "2.0");
        assert_eq!(str_dbl(0.0), "0.0");
        assert_eq!(str_dbl(-2.6805), "-2.6805");
    }

    #[test]
    fn str_dbl_e_matches_mmana() {
        assert_eq!(str_dbl_e(8.0e-4), "8.000e-04");
        assert_eq!(str_dbl_e(-1.25e-5), "-1.250e-05");
        assert_eq!(str_dbl_e(0.0254), "0.0254");
    }
}
