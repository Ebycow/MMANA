//! Wire expansion and automatic segmentation (`SetWZ`, `Tei`, `TeiB`, `TeiE`,
//! `TeiAvg`, `CombWire` in ComLib.cpp).
//!
//! Each defined wire becomes one or more expanded wires (`WzDef`) with a
//! segment count; tapering puts short segments at the wire ends.

use super::antdef::{AntDef, PDef, WDef, WzDef};
use super::{PPMAX, WZMAX};
use crate::error::CalcError;

/// `DM2DEF`: tolerance before an extra segment pair is added.
const DM2DEF: f64 = 0.85;

impl AntDef {
    /// Keeps at most one wire past `WZMAX` (the original drops the excess
    /// silently), so [`AntDef::set_wz`] can report the overflow.
    fn add_wz(&mut self, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, r: f64, seg: i32, wno: usize) {
        if self.wz.len() <= WZMAX {
            self.wz.push(WzDef { x1, y1, z1, x2, y2, z2, r, seg, wno, pno: 0, sno: 0, smax: 0 });
        }
    }

    /// `TeiAvg`: equal segments of at most λ/dm, always an even count.
    fn tei_avg(&mut self, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, r: f64, dm: i32, wno: usize) {
        let r = r.abs();
        let (xx, yy, zz) = (x2 - x1, y2 - y1, z2 - z1);
        let d = (xx * xx + yy * yy + zz * zz).sqrt();
        let mut n = (d / (self.rmd / dm as f64)) as i32;
        // down to even; the original's `&= 0xfffe` also drops everything from
        // 65536 up, so 65536 segments became 4 instead of over the limit
        n &= !1;
        if n == 0 {
            n += 2;
        }
        if n <= 4 && (d / n as f64) * DM2DEF > self.rmd / dm as f64 {
            n += 2;
        }
        self.add_wz(x1, y1, z1, x2, y2, z2, r, n, wno);
    }

    /// `Tei`: tapering at both ends.
    fn tei(&mut self, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, r: f64, dm1_in: i32, wno: usize) {
        let dm1 = if dm1_in == 1 { self.dm1 } else { dm1_in } as f64;
        let mut dm2 = self.dm2 as f64;
        let (sc, ec, rmd) = (self.sc, self.ec, self.rmd);
        let r = r.abs();
        let (xx, yy, zz) = (x2 - x1, y2 - y1, z2 - z1);
        let d = (xx * xx + yy * yy + zz * zz).sqrt();

        while dm2 < dm1 {
            let mut x = 0.0;
            let mut nt = 0;
            let mut dm = dm1;
            while dm > dm2 {
                if nt == 0 {
                    x += (rmd * ec as f64) / dm;
                } else {
                    x += rmd / dm;
                }
                dm /= sc;
                nt += 1;
            }
            let mut dd = d - (x * 2.0);
            let mut n = (dd / (rmd / dm2)) as i32;
            n &= !1;
            if dd > 0.0 && n >= 2 {
                dd /= n as f64;
                let (mut xc, mut yc, mut zc) = (x1, y1, z1);
                let mut nl = 0;
                dm = dm1;
                while dm > dm2 {
                    let (xxc, yyc, zzc) = (xc, yc, zc);
                    let x = rmd * (if nl != 0 { 1 } else { ec }) as f64 / dm;
                    xc += xx * x / d;
                    yc += yy * x / d;
                    zc += zz * x / d;
                    self.add_wz(xxc, yyc, zzc, xc, yc, zc, r, if nl != 0 { 1 } else { ec }, wno);
                    dm /= sc;
                    nl += 1;
                }
                if n != 0 {
                    let (xxc, yyc, zzc) = (xc, yc, zc);
                    xc += (xx * dd * n as f64) / d;
                    yc += (yy * dd * n as f64) / d;
                    zc += (zz * dd * n as f64) / d;
                    // (the original adds 2 segments when `(dd / n) * DM2DEF`
                    // passes λ / dm2 and n <= 4, but `dd` is already one
                    // segment, under 2 λ / dm2, so it never does)
                    self.add_wz(xxc, yyc, zzc, xc, yc, zc, r, n, wno);
                }
                dm *= sc;
                while nl != 0 {
                    let (xxc, yyc, zzc) = (xc, yc, zc);
                    let x = rmd / dm;
                    xc += xx * x / d;
                    yc += yy * x / d;
                    zc += zz * x / d;
                    if nl == 1 {
                        self.add_wz(xxc, yyc, zzc, x2, y2, z2, r, ec, wno);
                    } else {
                        self.add_wz(xxc, yyc, zzc, xc, yc, zc, r, 1, wno);
                    }
                    dm *= sc;
                    nl -= 1;
                }
                return;
            }
            dm2 *= 2.0;
        }
        self.tei_avg(x1, y1, z1, x2, y2, z2, r, dm1 as i32, wno);
    }

    /// `TeiB`: tapering at the start only.
    fn tei_b(&mut self, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, r: f64, dm1_in: i32, wno: usize) {
        let dm1 = if dm1_in == 1 { self.dm1 } else { dm1_in } as f64;
        let mut dm2 = self.dm2 as f64;
        let (sc, ec, rmd) = (self.sc, self.ec, self.rmd);
        let r = r.abs();
        let (xx, yy, zz) = (x2 - x1, y2 - y1, z2 - z1);
        let d = (xx * xx + yy * yy + zz * zz).sqrt();

        while dm2 < dm1 {
            let mut x = 0.0;
            let mut nt = 0;
            let mut dm = dm1;
            while dm > dm2 {
                if nt == 0 {
                    x += (rmd * ec as f64) / dm;
                } else {
                    x += rmd / dm;
                }
                dm /= sc;
                nt += 1;
            }
            let dd = d - x;
            let mut n = (dd / (rmd / dm2)) as i32;
            if dd > 0.0 && n != 0 {
                let (mut xc, mut yc, mut zc) = (x1, y1, z1);
                dm = dm1;
                while dm > dm2 {
                    let (xxc, yyc, zzc) = (xc, yc, zc);
                    let x = rmd * (if dm == dm1 { ec } else { 1 }) as f64 / dm;
                    xc += xx * x / d;
                    yc += yy * x / d;
                    zc += zz * x / d;
                    self.add_wz(xxc, yyc, zzc, xc, yc, zc, r, if dm == dm1 { ec } else { 1 }, wno);
                    dm /= sc;
                }
                // a wire too long for i32 already has `n` at i32::MAX; the
                // pulse limit rejects it, so it must not wrap to a negative count
                if (dd / n as f64) * DM2DEF > rmd / self.dm2 as f64 {
                    n = n.saturating_add(1);
                }
                self.add_wz(xc, yc, zc, x2, y2, z2, r, n, wno);
                return;
            }
            dm2 *= 1.25;
        }
        self.tei_avg(x1, y1, z1, x2, y2, z2, r, dm1 as i32, wno);
    }

    /// `TeiE`: tapering at the end only.
    fn tei_e(&mut self, x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64, r: f64, dm1_in: i32, wno: usize) {
        let dm1 = if dm1_in == 1 { self.dm1 } else { dm1_in } as f64;
        let mut dm2 = self.dm2 as f64;
        let (sc, ec, rmd) = (self.sc, self.ec, self.rmd);
        let r = r.abs();
        let (xx, yy, zz) = (x2 - x1, y2 - y1, z2 - z1);
        let d = (xx * xx + yy * yy + zz * zz).sqrt();

        while dm2 < dm1 {
            let mut x = 0.0;
            let mut nt = 0;
            let mut dm = dm1;
            while dm > dm2 {
                if nt == 0 {
                    x += (rmd * ec as f64) / dm;
                } else {
                    x += rmd / dm;
                }
                dm /= sc;
                nt += 1;
            }
            let dd = d - x;
            let mut n = (dd / (rmd / dm2)) as i32;
            if dd > 0.0 && n != 0 {
                let mut xc = x1 + (xx * dd / d);
                let mut yc = y1 + (yy * dd / d);
                let mut zc = z1 + (zz * dd / d);
                // a wire too long for i32 already has `n` at i32::MAX; the
                // pulse limit rejects it, so it must not wrap to a negative count
                if (dd / n as f64) * DM2DEF > rmd / self.dm2 as f64 {
                    n = n.saturating_add(1);
                }
                self.add_wz(x1, y1, z1, xc, yc, zc, r, n, wno);
                dm *= sc;
                while nt != 0 {
                    let (xxc, yyc, zzc) = (xc, yc, zc);
                    let x = rmd / dm;
                    xc += xx * x / d;
                    yc += yy * x / d;
                    zc += zz * x / d;
                    if nt == 1 {
                        self.add_wz(xxc, yyc, zzc, x2, y2, z2, r, ec, wno);
                    } else {
                        self.add_wz(xxc, yyc, zzc, xc, yc, zc, r, 1, wno);
                    }
                    dm *= sc;
                    nt -= 1;
                }
                return;
            }
            dm2 *= 1.25;
        }
        self.tei_avg(x1, y1, z1, x2, y2, z2, r, dm1 as i32, wno);
    }

    /// `SetWZ(ap, i, wp)`: expands one (non-combined) wire.
    fn set_wz_one(&mut self, i: usize, wp: &WDef) {
        let (x1, y1, z1, x2, y2, z2, r) = (wp.x1, wp.y1, wp.z1, wp.x2, wp.y2, wp.z2, wp.r);
        if wp.seg > 0 {
            self.add_wz(x1, y1, z1, x2, y2, z2, r, wp.seg, i);
        } else if wp.seg < 0 {
            match wp.seg {
                -2 => self.tei_b(x1, y1, z1, x2, y2, z2, r, self.dm1, i),
                -3 => self.tei_e(x1, y1, z1, x2, y2, z2, r, self.dm1, i),
                seg if seg < -10000 => {
                    let save_dm2 = self.dm2;
                    self.dm2 = (seg >> 16) & 0x7fff;
                    self.tei(x1, y1, z1, x2, y2, z2, r, seg & 0x7fff, i);
                    self.dm2 = save_dm2;
                }
                seg => self.tei(x1, y1, z1, x2, y2, z2, r, -seg, i),
            }
        } else {
            self.tei_avg(x1, y1, z1, x2, y2, z2, r, self.dm2, i);
        }
    }

    /// `SetWZ(ap)`: expands every wire. `rmd` must already hold the wavelength.
    pub fn set_wz(&mut self) -> Result<(), CalcError> {
        self.min_z = f64::MAX;
        self.wz.clear();
        for w in &mut self.wires {
            w.pno = 0;
            w.pmax = 0;
        }
        for i in 0..self.wires.len() {
            let wp = self.wires[i].clone();
            if wp.r == 0.0 {
                continue;
            }
            if self.wz.len() >= WZMAX {
                return Err(CalcError::TooManySegmentWires { max: WZMAX });
            }
            self.min_z = self.min_z.min(wp.z1).min(wp.z2);
            if wp.r < 0.0 {
                let Some(pp) = self.pdef.iter().find(|p| p.rr == wp.r).cloned() else {
                    return Err(CalcError::CombinedWireNotFound { wire: i + 1, radius: wp.r });
                };
                for part in comb_wire(self, &pp, &wp) {
                    self.set_wz_one(i, &part);
                }
            } else {
                self.set_wz_one(i, &wp);
            }
            // a tapered or combined wire can cross the limit part-way through
            if self.wz.len() > WZMAX {
                return Err(CalcError::TooManySegmentWires { max: WZMAX });
            }
        }
        Ok(())
    }
}

/// `GetLen`: span covered by the centre section plus `n` pairs.
fn get_len(pp: &PDef, n: usize) -> f64 {
    let mut d = pp.l[0];
    for i in 1..=n {
        if pp.l[i] != 0.0 {
            d += pp.l[i] * 2.0;
        } else {
            d = 1.0e20;
            break;
        }
    }
    d
}

fn wire_len(w: &WDef) -> f64 {
    let (xx, yy, zz) = (w.x2 - w.x1, w.y2 - w.y1, w.z2 - w.z1);
    (xx * xx + yy * yy + zz * zz).sqrt()
}

/// `CombWire`: splits a wire into sections of a [`PDef`] (telescoping tube).
pub(super) fn comb_wire(ap: &AntDef, pp: &PDef, wp: &WDef) -> Vec<WDef> {
    let xx = wp.x2 - wp.x1;
    let yy = wp.y2 - wp.y1;
    let zz = wp.z2 - wp.z1;
    let mut d = (xx * xx + yy * yy + zz * zz).sqrt();
    let dd = d;
    let mut tbl = vec![WDef::default(); PPMAX * 2 + 2];
    tbl[0].x1 = wp.x1;
    tbl[0].y1 = wp.y1;
    tbl[0].z1 = wp.z1;
    let mut n = 0usize;
    // L beyond the array aliases R[0] in the original struct layout.
    let l_at = |j: usize| if j < PPMAX { pp.l[j] } else { pp.r[0] };
    let r_at = |j: usize| if j < PPMAX { pp.r[j] } else { 0.0 };
    let section_seg = |wseg: i32| {
        if pp.ty & 2 != 0 {
            if wseg >= 0 { wseg } else { 0 }
        } else if wseg == -2 || wseg == -3 {
            0
        } else {
            wseg
        }
    };
    let advance = |tbl: &mut Vec<WDef>, n: usize, l: f64| {
        let x = tbl[n].x1 + (xx * l) / dd;
        let y = tbl[n].y1 + (yy * l) / dd;
        let z = tbl[n].z1 + (zz * l) / dd;
        tbl[n].x2 = x;
        tbl[n].y2 = y;
        tbl[n].z2 = z;
        tbl[n + 1].x1 = x;
        tbl[n + 1].y1 = y;
        tbl[n + 1].z1 = z;
    };

    match pp.ty & 1 {
        0 => 'centre: {
            let mut i = 0;
            while i < PPMAX {
                if d <= get_len(pp, i) {
                    break;
                }
                i += 1;
            }
            if i == 0 {
                break 'centre;
            }
            let mut l = (dd - get_len(pp, i - 1)) / 2.0;
            for j in (0..i).rev() {
                advance(&mut tbl, n, l);
                tbl[n].r = if l_at(j + 1) != 0.0 { r_at(j + 1) } else { pp.r[j] };
                tbl[n].seg = section_seg(wp.seg);
                l = pp.l[j];
                n += 1;
            }
            // `AdjPdef` ends every list with a 99999.9 m section, so none up
            // to I has length 0 (the original falls back to the radius before
            // it, or 8 mm), and the last ends at the wire's end below: the
            // length the original counts down to cut it there changes nothing
            for j in 0..=i {
                advance(&mut tbl, n, l_at(j));
                tbl[n].r = r_at(j);
                tbl[n].seg = section_seg(wp.seg);
                n += 1;
            }
        }
        _ => {
            let mut i = 0;
            // (no section has length 0, as above)
            while i < PPMAX && d != 0.0 {
                let l = pp.l[i].min(d);
                advance(&mut tbl, n, l);
                tbl[n].r = pp.r[i];
                tbl[n].seg = section_seg(wp.seg);
                d -= l;
                n += 1;
                i += 1;
            }
        }
    }

    if n != 0 {
        let i = n - 1;
        tbl[i].x2 = wp.x2;
        tbl[i].y2 = wp.y2;
        tbl[i].z2 = wp.z2;
        let dm_len = ap.rmd / ap.dm2 as f64;
        if wp.seg < 0 {
            match wp.seg {
                -2 => {
                    if wire_len(&tbl[0]) < dm_len {
                        tbl[0].seg = -1;
                        tbl[1].seg = -2;
                    } else {
                        tbl[0].seg = -2;
                    }
                }
                -3 => {
                    if i != 0 && wire_len(&tbl[0]) < dm_len {
                        tbl[i - 1].seg = -3;
                        tbl[i].seg = -1;
                    } else {
                        tbl[i].seg = -3;
                    }
                }
                _ => {
                    if i == 0 {
                        tbl[0].seg = wp.seg;
                    } else if pp.ty & 2 != 0 {
                        if wire_len(&tbl[0]) < dm_len {
                            tbl[0].seg = -1;
                            tbl[1].seg = if n > 3 { -2 } else { -1 };
                        } else {
                            tbl[0].seg = -2;
                        }
                        if i != 0 && wire_len(&tbl[0]) < dm_len {
                            tbl[i - 1].seg = if n > 3 { -3 } else { -1 };
                            tbl[i].seg = -1;
                        } else {
                            tbl[i].seg = -3;
                        }
                    }
                }
            }
        }
        if pp.ty & 1 == 0 && n >= 3 {
            for i in 0..n {
                if tbl[i].seg == 0 && i != n / 2 {
                    let d = wire_len(&tbl[i]);
                    let mut nn = (d / dm_len) as i32;
                    if nn == 0 {
                        nn += 1;
                    }
                    if nn <= 4 && (d / nn as f64) * DM2DEF > dm_len {
                        nn += 1;
                    }
                    tbl[i].seg = nn;
                }
            }
        }
    } else {
        tbl[0] = wp.clone();
        tbl[0].r = pp.r[0];
        n = 1;
    }
    tbl.truncate(n);
    tbl
}
