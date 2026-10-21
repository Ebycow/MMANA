//! Pulse-name resolution (`Str2PlusNo`, `AdjPlusNo` in ComLib.cpp).

use super::antdef::AntDef;

impl AntDef {
    /// `AdjPlusNo`: derives each wire's first pulse / pulse count from the
    /// expanded wires, then resolves source and load pulse names.
    pub fn adj_plus_no(&mut self) {
        for zp in &self.wz {
            if zp.pno != 0 {
                let wp = &mut self.wires[zp.wno];
                if wp.pno == 0 {
                    wp.pno = zp.pno;
                }
                wp.pmax += (zp.smax - zp.sno) + 1;
            }
        }
        for i in 0..self.sources.len() {
            self.sources[i].plus_no = self.copy_pulse_no(&self.sources[i].plus, self.sources[i].copy);
        }
        for i in 0..self.loads.len() {
            self.loads[i].plus_no = self.copy_pulse_no(&self.loads[i].plus, self.loads[i].copy);
        }
    }

    /// [`AntDef::pulse_no`] in stacked copy `copy`. A wire name already
    /// carries the copy's wire number; a bare number names a pulse of the
    /// first copy and becomes the pulse as far along the same wire of this
    /// copy (0 if the first copy has no such pulse).
    fn copy_pulse_no(&self, name: &str, copy: usize) -> i32 {
        let p = self.pulse_no(name);
        if copy == 0 || !to_half_width(name).starts_with(|c: char| c.is_ascii_digit()) {
            return p;
        }
        let Some((w, wp)) =
            self.wires[..self.base_wires].iter().enumerate().find(|(_, wp)| (wp.pno..wp.pno + wp.pmax).contains(&p))
        else {
            return 0;
        };
        self.wires.get(w + copy * self.base_wires).map_or(0, |cw| cw.pno + (p - wp.pno))
    }

    /// `Str2PlusNo`: `12` → pulse 12, `w3c` → centre of wire 3, `w3b2` → 2nd
    /// pulse from the beginning, `w3e1` → 1 before the end. 0 means invalid.
    pub fn pulse_no(&self, name: &str) -> i32 {
        let bf = to_half_width(name).to_ascii_uppercase();
        let b = bf.as_bytes();
        match b.first() {
            Some(c) if c.is_ascii_digit() => leading_uint(&bf).unwrap_or(0),
            Some(b'W') => {
                let digits = b[1..].iter().take_while(|c| c.is_ascii_digit()).count();
                let wire_part = &bf[1..1 + digits];
                // the kind letter must be ASCII; anything else is an invalid name
                let (kind, rest) = match bf[1 + digits..].chars().next() {
                    Some(c) if c.is_ascii() => (c as u8, &bf[2 + digits..]),
                    Some(_) => return 0,
                    None => (b'C', ""),
                };
                let n = if rest.is_empty() { Some(0) } else { leading_int(rest) };
                let mut w: i64 = 0;
                if !wire_part.is_empty() {
                    // a number past i32 is no wire, not the wire it wraps to
                    let Some(no) = leading_uint(wire_part) else { return 0 };
                    w = no as i64;
                    if w != 0 {
                        w -= 1;
                    }
                }
                // a wire past the last is no wire
                let Some(wp) = self.wires.get(w as usize) else { return 0 };
                let Some(n) = n else { return 0 };
                match kind {
                    b'C' => (wp.pmax / 2 + wp.pno).checked_add(n),
                    b'B' => wp.pno.checked_add(n),
                    b'E' => (wp.pno + wp.pmax).checked_sub(n),
                    _ => None,
                }
                .unwrap_or(0)
            }
            _ => 0,
        }
    }
}

/// Converts full-width ASCII (U+FF01-FF5E) and the ideographic space to
/// half-width, as MMANA does when it reads pulse names.
pub(crate) fn to_half_width(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{3000}' => ' ',
            c => c,
        })
        .collect()
}

/// `sscanf("%u")` on the leading digits: 0 without digits, `None` past
/// `i32::MAX` (the original wraps such a number onto a small one).
fn leading_uint(s: &str) -> Option<i32> {
    let digits: String = s.trim_start().chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() { Some(0) } else { digits.parse::<i32>().ok() }
}

/// `sscanf("%d")` on the leading signed integer.
fn leading_int(s: &str) -> Option<i32> {
    let s = s.trim_start();
    let (sign, body) = match s.as_bytes().first() {
        Some(b'-') => (-1, &s[1..]),
        Some(b'+') => (1, &s[1..]),
        _ => (1, s),
    };
    leading_uint(body).map(|v| sign * v)
}

#[cfg(test)]
mod tests {
    use super::super::antdef::WDef;
    use super::*;

    /// Two wires: pulses 1-9 and 10-19.
    fn two_wires() -> AntDef {
        AntDef {
            cfq: 14.0,
            wires: vec![WDef { pno: 1, pmax: 9, ..WDef::default() }, WDef { pno: 10, pmax: 10, ..WDef::default() }],
            base_wires: 2,
            sources: Vec::new(),
            loads_enabled: false,
            loads: Vec::new(),
            pdef: Vec::new(),
            wz: Vec::new(),
            min_z: 0.0,
            rmd: 0.0,
            dm1: 400,
            dm2: 40,
            ec: 1,
            sc: 2.0,
        }
    }

    #[test]
    fn resolves_names() {
        let ap = two_wires();
        assert_eq!(ap.pulse_no("12"), 12);
        assert_eq!(ap.pulse_no("w1c"), 5);
        assert_eq!(ap.pulse_no("W2"), 15);
        assert_eq!(ap.pulse_no("w2b2"), 12);
        assert_eq!(ap.pulse_no("w2e1"), 19);
        assert_eq!(ap.pulse_no("w3c"), 0);
    }

    #[test]
    fn numbers_resolve_along_the_same_wire_of_a_stacked_copy() {
        let mut ap = two_wires();
        ap.wires.push(WDef { pno: 20, pmax: 9, ..WDef::default() });
        ap.wires.push(WDef { pno: 29, pmax: 11, ..WDef::default() });
        assert_eq!(ap.copy_pulse_no("5", 0), 5);
        assert_eq!(ap.copy_pulse_no("5", 1), 24);
        assert_eq!(ap.copy_pulse_no("１２", 1), 31);
        assert_eq!(ap.copy_pulse_no("19", 1), 38);
        // not a pulse of the first copy
        assert_eq!(ap.copy_pulse_no("25", 1), 0);
        assert_eq!(ap.copy_pulse_no("0", 1), 0);
        // the copy's wire number is already in the name
        assert_eq!(ap.copy_pulse_no("w3c", 1), 24);
    }

    #[test]
    fn numbers_past_i32_are_invalid_not_wrapped() {
        let ap = two_wires();
        // 2^32 + 1 wraps to 1
        assert_eq!(ap.pulse_no("4294967297"), 0);
        assert_eq!(ap.pulse_no("w4294967297c"), 0);
        assert_eq!(ap.pulse_no("w1b4294967297"), 0);
        assert_eq!(ap.pulse_no("w1c2147483647"), 0);
        assert_eq!(ap.pulse_no("w1e-2147483648"), 0);
        assert_eq!(ap.pulse_no("w2b2147483647"), 0);
    }

    #[test]
    fn full_width_names_resolve_like_half_width() {
        let ap = two_wires();
        assert_eq!(ap.pulse_no("ｗ１ｃ"), 5);
        assert_eq!(ap.pulse_no("w１c"), 5);
        assert_eq!(ap.pulse_no("w2b２"), 12);
        assert_eq!(ap.pulse_no("１２"), 12);
        // trailing text after the offset is ignored, as with sscanf
        assert_eq!(ap.pulse_no("w1c２あ"), 7);
    }

    #[test]
    fn non_ascii_names_are_invalid_not_a_panic() {
        let ap = two_wires();
        for name in ["wあ", "w1あ", "wあ1c", "w🙂", "あ", "w1é", ""] {
            assert_eq!(ap.pulse_no(name), 0, "{name}");
        }
    }

    #[test]
    fn half_width_conversion() {
        assert_eq!(to_half_width("ｗ１ｃ"), "w1c");
        assert_eq!(to_half_width("Ｗ３Ｂ－１　"), "W3B-1 ");
        assert_eq!(to_half_width("給電ﾜｲﾔ"), "給電ﾜｲﾔ");
    }
}
