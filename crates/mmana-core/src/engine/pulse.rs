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
            self.sources[i].plus_no = self.pulse_no(&self.sources[i].plus);
        }
        for i in 0..self.loads.len() {
            self.loads[i].plus_no = self.pulse_no(&self.loads[i].plus);
        }
    }

    /// `Str2PlusNo`: `12` → pulse 12, `w3c` → centre of wire 3, `w3b2` → 2nd
    /// pulse from the beginning, `w3e1` → 1 before the end. 0 means invalid.
    pub fn pulse_no(&self, name: &str) -> i32 {
        let bf = name.to_ascii_uppercase();
        let b = bf.as_bytes();
        match b.first() {
            Some(c) if c.is_ascii_digit() => leading_uint(&bf),
            Some(b'W') => {
                let digits = b[1..].iter().take_while(|c| c.is_ascii_digit()).count();
                let wire_part = &bf[1..1 + digits];
                let (kind, rest) = match b.get(1 + digits) {
                    Some(&c) => (c, &bf[2 + digits..]),
                    None => (b'C', ""),
                };
                let n = if rest.is_empty() { 0 } else { leading_int(rest) };
                let mut w: i64 = 0;
                if !wire_part.is_empty() {
                    w = leading_uint(wire_part) as i64;
                    if w != 0 {
                        w -= 1;
                    }
                    if w < 0 || w >= self.wires.len() as i64 {
                        return 0;
                    }
                }
                let Some(wp) = self.wires.get(w as usize) else { return 0 };
                match kind {
                    b'C' => (wp.pmax / 2) + wp.pno + n,
                    b'B' => wp.pno + n,
                    b'E' => wp.pno + wp.pmax - n,
                    _ => 0,
                }
            }
            _ => 0,
        }
    }
}

/// `sscanf("%u")` on the leading digits.
fn leading_uint(s: &str) -> i32 {
    let digits: String = s.trim_start().chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<i64>().map_or(0, |v| v as i32)
}

/// `sscanf("%d")` on the leading signed integer.
fn leading_int(s: &str) -> i32 {
    let s = s.trim_start();
    let (sign, body) = match s.as_bytes().first() {
        Some(b'-') => (-1, &s[1..]),
        Some(b'+') => (1, &s[1..]),
        _ => (1, s),
    };
    sign * leading_uint(body)
}
