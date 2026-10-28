//! S497 / S572 / S607: per-pulse current magnitudes as MMANA displays them
//! (signed by the dominant current direction), including junction currents
//! at wire ends (`CRes::SetCur` / `FinishCur`).

use super::Mininec;
use super::antdef::AntDef;

/// `NULLF`: "no value" marker of the display array.
pub const NULLF: f32 = 1.2345e-37;

/// Mirrors the current part of `CRes`.
struct CurState {
    cur: Vec<f32>,
    dir: Vec<u8>,
    real_max: f64,
    img_max: f64,
    cur_n: usize,
}

impl CurState {
    fn set_cur_dir(&mut self, n: usize, mut r: f64, mut j: f64) {
        if r < 0.0 {
            self.dir[n] |= 0x01;
            r = -r;
        }
        if j < 0.0 {
            self.dir[n] |= 0x02;
            j = -j;
        }
        if r > self.real_max {
            self.real_max = r;
        }
        if j > self.img_max {
            self.img_max = j;
        }
        self.cur_n = n;
    }

    fn set_cur(&mut self, n: usize, r: f64, j: f64, d: f64) {
        self.set_cur_dir(n, r, j);
        self.cur[n] = d as f32;
    }

    /// `CRes::SetCur(ap, ...)`: value for the first pulse of expanded wire `n`
    /// unless already set.
    fn set_cur_wire(&mut self, ap: &AntDef, n: usize, r: f64, j: f64, d: f64) {
        if let Some(wz) = ap.wz.get(n) {
            if wz.pno != 0 {
                let p = (wz.pno - 1) as usize;
                if self.cur[p] == NULLF {
                    self.set_cur_dir(p, r, j);
                    self.cur[p] = d as f32;
                }
            }
        }
    }

    fn finish(mut self) -> Vec<f32> {
        let mask = if self.real_max > self.img_max { 0x01 } else { 0x02 };
        for i in 0..=self.cur_n {
            if self.dir[i] & mask != 0 {
                self.cur[i] = -self.cur[i];
            }
        }
        self.cur.truncate(self.cur_n + 1);
        self.cur
    }
}

/// S607: magnitude of a complex current.
fn magnitude(i2f: f64, j2f: f64) -> f64 {
    let i3f = i2f * i2f;
    let j3f = j2f * j2f;
    if i3f > 0.0 || j3f > 0.0 { (i3f + j3f).sqrt() } else { 0.0 }
}

impl Mininec {
    fn ci_at(&self, row: i32, e: usize) -> i32 {
        if row < 0 { 0 } else { self.ci.get(row as usize).map_or(0, |c| c[e]) }
    }

    fn cr_at(&self, row: i32) -> (f64, f64) {
        if row < 0 {
            return (0.0, 0.0);
        }
        let r = row as usize;
        (self.cr.get(r).copied().unwrap_or(0.0), self.cim.get(r).copied().unwrap_or(0.0))
    }

    /// S572: sorts junction currents at end `ei` (1 or 2) of wire `k`.
    /// Returns (I1f, J1f, is_junction).
    fn s572(&self, k: usize, c: f64, i: i32, ei: i32) -> (f64, f64, bool) {
        let ki = k as i32;
        let mut junction = false;
        let (mut i1f, mut j1f) = (0.0, 0.0);
        if !(c == ki as f64 || c == 0.0) {
            junction = true;
            (i1f, j1f) = self.cr_at(i);
        }
        // check for other overlapping wires
        for j in 1..=self.nw {
            if j == k {
                continue;
            }
            let (l1, l2) = (self.nn[j][1], self.nn[j][2]);
            let (co, ct, l3, l4) = if ei == 2 {
                (self.ci_at(l2, 2), self.ci_at(l1, 1), l2, l1)
            } else {
                (self.ci_at(l1, 1), self.ci_at(l2, 2), l1, l2)
            };
            if co == -ki {
                let (r, im) = self.cr_at(l3);
                i1f -= r;
                j1f -= im;
                junction = true;
            }
            if ct == ki {
                let (r, im) = self.cr_at(l4);
                i1f += r;
                j1f += im;
                junction = true;
            }
        }
        (i1f, j1f, junction)
    }

    /// S497: current distribution, index = pulse - 1.
    pub fn current_distribution(&self, ap: &AntDef) -> Vec<f32> {
        let len = self.n.max(1) + 1;
        let mut st = CurState { cur: vec![NULLF; len], dir: vec![0; len], real_max: 0.0, img_max: 0.0, cur_n: 0 };
        for k in 1..=self.nw {
            let ki = k as i32;
            let mut n1 = self.nn[k][1];
            let mut n2 = self.nn[k][2];
            let mut c = self.ci_at(n1, 1) as f64;
            if n1 == 0 && n2 == 0 {
                c = ki as f64;
            }
            let mut to_525 = false;
            if self.g != 1 {
                if self.j1[k] == -1 && n1 > n2 {
                    n2 = n1;
                }
                if self.j1[k] == -1 {
                    to_525 = true;
                }
            }
            let mut skip_pulses = false;
            if !to_525 {
                let (i1f, j1f, junction) = self.s572(k, c, n1, 1);
                st.set_cur_wire(ap, k - 1, i1f, j1f, magnitude(i1f, j1f));
                if n1 == 0 {
                    skip_pulses = true;
                } else if c != ki as f64 && junction {
                    n1 += 1;
                }
            }
            // L525
            if !skip_pulses {
                for i in n1..n2 {
                    let (r, im) = self.cr_at(i);
                    st.set_cur((i - 1) as usize, r, im, magnitude(r, im));
                }
            }
            // L532 (the original sets C to K when the wire has no pulses,
            // where L537 goes on to L543 whatever C is)
            let at_543 = if self.g != 1 && self.j1[k] == 1 {
                true
            } else if (n1 == 0 && n2 == 0) || n1 > n2 {
                false
            } else {
                // L537
                let c = self.ci_at(n2, 2) as f64;
                let (_, _, junction) = self.s572(k, c, n2, 2);
                c == ki as f64 || !junction
            };
            if at_543 && n2 >= 1 {
                let (r, im) = self.cr_at(n2);
                st.set_cur((n2 - 1) as usize, r, im, magnitude(r, im));
            }
        }
        st.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_current_in_phase_has_its_real_part_as_magnitude() {
        assert_eq!(magnitude(2.0, 0.0), 2.0);
        assert_eq!(magnitude(0.0, -3.0), 3.0);
        assert_eq!(magnitude(0.0, 0.0), 0.0);
    }
}
