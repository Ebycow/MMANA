//! Geometry, connectivity and excitation set-up (`InitAntData`, `S1299`).

use super::antdef::AntDef;
use super::circuit::{calc_para_z, wire_loss};
use super::{EngineConfig, LoadDef, Mininec, PAI2, SMAX, SourceDef, sgn};
use crate::error::CalcError;
use crate::model::{GroundKind, WireMaterial};

const CQ: [f64; 15] = [
    0.0,
    0.288675135,
    0.5,
    0.430568156,
    0.173927423,
    0.169990522,
    0.326072577,
    0.480144928,
    0.050614268,
    0.398333239,
    0.111190517,
    0.262766205,
    0.156853323,
    0.091717321,
    0.181341892,
];

type P3 = [f64; 3];

impl Mininec {
    /// `InitAntData`: expands and segments the wires, builds pulses,
    /// connectivity, loads and sources. Updates `ap` (segment wires and pulse
    /// numbers) the way the original mutates `ANTDEF`.
    pub fn new(ap: &mut AntDef, cfg: &EngineConfig) -> Result<Self, CalcError> {
        let p = 4.0 * 1f64.atan();
        let mut f = ap.cfq;
        if f == 0.0 {
            f = 299.8;
        }
        let wavelength = 299.8 / f;
        let w = 2.0 * p / wavelength;
        ap.rmd = wavelength;

        let mut e = Mininec {
            p,
            p0: p / 180.0,
            g0: 29.979221,
            q: CQ,
            c: [
                1.38629436112,
                0.09666344259,
                0.03590092383,
                0.03742563713,
                0.01451196212,
                0.5,
                0.12498593397,
                0.06880248576,
                0.0332835346,
                0.00441787012,
            ],
            f,
            wavelength,
            w,
            w2: w * w / 2.0,
            s0: 0.001 * wavelength,
            m: 4.77783352 * wavelength,
            srm: 0.0001 * wavelength,
            g: 1,
            nm: 0,
            tb: 1,
            nr: 0,
            rr: 0.0,
            med_t: vec![0.0],
            med_v: vec![0.0],
            med_u: vec![0.0],
            med_h: vec![0.0],
            height_off: 0.0,
            nw: 0,
            a: Vec::new(),
            ca: Vec::new(),
            cb: Vec::new(),
            cg: Vec::new(),
            s: Vec::new(),
            j1: Vec::new(),
            j2: Vec::new(),
            nn: Vec::new(),
            n: 0,
            x: Vec::new(),
            y: Vec::new(),
            z: Vec::new(),
            ci: Vec::new(),
            wi: Vec::new(),
            sources: Vec::new(),
            loads: Vec::new(),
            swr_ro: cfg.swr_ro,
            swr_jxo: cfg.swr_jxo,
            zr: Vec::new(),
            zi: Vec::new(),
            cr: Vec::new(),
            cim: Vec::new(),
            pwr: 0.0,
            solutions: Vec::new(),
        };

        ap.set_wz()?;
        e.set_ground(cfg);
        e.build_wires(ap, cfg)?;
        e.number_pulses(ap);
        ap.adj_plus_no();
        e.set_loads(ap, cfg)?;
        e.set_sources(ap)?;
        Ok(e)
    }

    fn set_ground(&mut self, cfg: &EngineConfig) {
        if cfg.ground == GroundKind::FreeSpace {
            self.g = 1;
            self.nm = 0;
            return;
        }
        self.g = -1;
        self.nm = 0;
        self.tb = 1;
        if cfg.ground == GroundKind::Real {
            self.nm = cfg.media.len();
            for m in &cfg.media {
                self.med_t.push(m.rel);
                self.med_v.push(m.cond);
                self.med_u.push(m.boundary);
                self.med_h.push(m.height);
            }
            if let Some((count, radius)) = cfg.screen {
                self.nr = count;
                self.rr = radius;
                self.tb = 2;
            }
            // the outermost medium always extends to "infinity"
            if self.nm > 0 && self.med_u[self.nm] < 1000000.0 {
                self.med_u[self.nm] = 1000000.0;
            }
        }
    }

    /// Wire loop of `InitAntData` (L1200-L1255).
    fn build_wires(&mut self, ap: &AntDef, cfg: &EngineConfig) -> Result<(), CalcError> {
        let nw = ap.wz.len();
        self.nw = nw;
        self.height_off = if cfg.ground != GroundKind::FreeSpace { cfg.height } else { 0.0 };
        let max_pulses: usize = ap.wz.iter().map(|w| w.seg.max(1) as usize).sum::<usize>() + 2 * nw + 4;
        self.a = vec![0.0; nw + 2];
        self.ca = vec![0.0; nw + 2];
        self.cb = vec![0.0; nw + 2];
        self.cg = vec![0.0; nw + 2];
        self.s = vec![0.0; nw + 2];
        self.j1 = vec![0; nw + 2];
        self.j2 = vec![[0; 3]; nw + 2];
        self.nn = vec![[0; 3]; nw + 2];
        self.x = vec![0.0; max_pulses + 2 * nw + 2];
        self.y = vec![0.0; max_pulses + 2 * nw + 2];
        self.z = vec![0.0; max_pulses + 2 * nw + 2];
        self.ci = vec![[0; 3]; max_pulses + 2];
        self.wi = vec![0; max_pulses + 2];
        let mut ends: Vec<(P3, P3)> = vec![([0.0; 3], [0.0; 3]); nw + 2];

        let mut n: i32 = 0;
        for i in 1..=nw {
            let wz = &ap.wz[i - 1];
            let s1 = wz.seg as f64;
            let (x1, y1, z1) = (wz.x1, wz.y1, wz.z1 + self.height_off);
            let (x2, y2, z2) = (wz.x2, wz.y2, wz.z2 + self.height_off);
            self.a[i] = wz.r;
            if x1 == x2 && y1 == y2 && z1 == z2 {
                return Err(CalcError::ZeroLengthWire { wire: wz.wno + 1 });
            }
            let (i1, i2) = self.connect(i, [x1, y1, z1], [x2, y2, z2], &mut ends);
            let x3 = x2 - x1;
            let y3 = y2 - y1;
            let z3 = z2 - z1;
            let d = (x3 * x3 + y3 * y3 + z3 * z3).sqrt();
            self.ca[i] = x3 / d;
            self.cb[i] = y3 / d;
            self.cg[i] = z3 / d;
            self.s[i] = d / s1;
            // connectivity data (pulses N1 to N)
            let n1 = n + 1;
            self.nn[i][1] = n1;
            if s1 == 1.0 && i1 == 0 {
                self.nn[i][1] = 0;
            }
            n = (n1 as f64 + s1) as i32;
            if i1 == 0 {
                n -= 1;
            }
            if i2 == 0 {
                n -= 1;
            }
            if n as usize > cfg.max_pulses {
                return Err(CalcError::TooManyPulses { max: cfg.max_pulses });
            }
            self.nn[i][2] = n;
            if s1 == 1.0 && i2 == 0 {
                self.nn[i][2] = 0;
            }
            let ii = i as i32;
            if n < n1 {
                // single segment, no pulse (L1247)
                let b = (n1 + 2 * (ii - 1)) as usize;
                self.set_xyz(b, x1, y1, z1);
                self.set_xyz(b + 1, x2, y2, z2);
                continue;
            }
            let (n1u, nu) = (n1 as usize, n as usize);
            for j in n1u..=nu {
                self.ci[j] = [0, ii, ii];
                self.wi[j] = ii;
            }
            self.ci[n1u][1] = i1;
            self.ci[nu][2] = i2;
            // coordinates of break points
            let b1 = (n1 + 2 * (ii - 1)) as usize;
            let mut b3 = b1;
            self.set_xyz(b1, x1, y1, z1);
            if self.ci[n1u][1] != 0 {
                let c = self.ci[n1u][1];
                let w2 = c.unsigned_abs() as usize;
                let mut f3 = sgn(c) as f64 * self.s[w2];
                self.x[b1] = self.x[b1] - f3 * self.ca[w2];
                self.y[b1] = self.y[b1] - f3 * self.cb[w2];
                if c == -ii {
                    f3 = -f3;
                }
                self.z[b1] = self.z[b1] - f3 * self.cg[w2];
                b3 += 1;
            }
            let b6 = nu + 2 * i;
            for b4 in b1 + 1..=b6 {
                let j = b4 as f64 - b3 as f64;
                self.x[b4] = x1 + j * x3 / s1;
                self.y[b4] = y1 + j * y3 / s1;
                self.z[b4] = z1 + j * z3 / s1;
            }
            if self.ci[nu][2] != 0 {
                let c = self.ci[nu][2];
                let w2 = c.unsigned_abs() as usize;
                let mut f3 = sgn(c) as f64 * self.s[w2];
                let b3 = b6 - 1;
                self.x[b6] = self.x[b3] + f3 * self.ca[w2];
                self.y[b6] = self.y[b3] + f3 * self.cb[w2];
                if ii == -c {
                    f3 = -f3;
                }
                self.z[b6] = self.z[b3] + f3 * self.cg[w2];
            }
        }
        if n <= 0 {
            return Err(CalcError::NoPulses);
        }
        self.n = n as usize;
        let len = self.n + 2;
        self.ci.truncate(len.max(2));
        self.wi.truncate(len.max(2));
        Ok(())
    }

    fn set_xyz(&mut self, i: usize, x: f64, y: f64, z: f64) {
        self.x[i] = x;
        self.y[i] = y;
        self.z[i] = z;
    }

    /// `S1299`: finds connections of wire `i` to the ground and to earlier
    /// wires. Returns (I1, I2): the connected wire at each end (negative when
    /// joined end-to-same-end, `-i` for ground, 0 when free).
    fn connect(&mut self, i: usize, p1: P3, p2: P3, ends: &mut [(P3, P3)]) -> (i32, i32) {
        ends[i] = (p1, p2);
        let ii = i as i32;
        let mut gi = false;
        let mut i1 = 0;
        let mut i2 = 0;
        self.j1[i] = 0;
        self.j2[i] = [0, -ii, -ii];
        let mut check_end1 = true;
        if self.g != 1 {
            // check for ground connection
            if p1[2] == 0.0 {
                i1 = -ii;
                self.j1[i] = -1;
                check_end1 = false;
            } else if p2[2] == 0.0 {
                i2 = -ii;
                self.j1[i] = 1;
                gi = true;
            }
        }
        if check_end1 {
            if i == 1 {
                return (i1, i2);
            }
            for j in 1..i {
                let jj = j as i32;
                if p1 == ends[j].0 {
                    // end1 to end1
                    i1 = -jj;
                    self.j2[i][1] = jj;
                    if self.j2[j][1] == -jj {
                        self.j2[j][1] = jj;
                    }
                    break;
                }
                if p1 == ends[j].1 {
                    // end1 to end2
                    i1 = jj;
                    self.j2[i][1] = jj;
                    if self.j2[j][2] == -jj {
                        self.j2[j][2] = jj;
                    }
                    break;
                }
            }
        }
        if gi || i == 1 {
            return (i1, i2);
        }
        for j in 1..i {
            let jj = j as i32;
            if p2 == ends[j].1 {
                // end2 to end2
                i2 = -jj;
                self.j2[i][2] = jj;
                if self.j2[j][2] == -jj {
                    self.j2[j][2] = jj;
                }
                break;
            }
            if p2 == ends[j].0 {
                // end2 to end1
                i2 = jj;
                self.j2[i][2] = jj;
                if self.j2[j][1] == -jj {
                    self.j2[j][1] = jj;
                }
                break;
            }
        }
        (i1, i2)
    }

    /// L1264-L1286: assigns first pulse / segment numbers to the expanded
    /// wires and fills unconnected pulse ends with their own wire.
    fn number_pulses(&mut self, ap: &mut AntDef) {
        enum At {
            L1270,
            L1279,
        }
        let nw = self.nw;
        let n = self.n;
        let nn = |k: usize, e: usize, s: &Self| s.nn.get(k).map_or(0, |v| v[e]);
        let mut k = 1usize;
        let mut j = 0usize;
        for i in 1..=n {
            let i1 = 2 * self.wi[i] - 1 + i as i32;
            let mut at = if k > nw || k == j { At::L1279 } else { At::L1270 };
            loop {
                match at {
                    At::L1270 => {
                        j = k;
                        if nn(k, 1, self) != 0 || nn(k, 2, self) != 0 {
                            at = At::L1279;
                            continue;
                        }
                        k += 1;
                        if k > nw {
                            break;
                        }
                    }
                    At::L1279 => {
                        if let Some(wz) = ap.wz.get_mut(k.wrapping_sub(1)) {
                            if wz.pno == 0 {
                                wz.pno = i as i32;
                            }
                            if wz.sno == 0 {
                                wz.sno = i1;
                            }
                            wz.smax = i1;
                        }
                        if i as i32 == nn(k, 2, self) || nn(k, 1, self) == nn(k, 2, self) || self.ci[i][2] == 0 {
                            k += 1;
                        }
                        if self.ci[i][1] == 0 {
                            self.ci[i][1] = self.wi[i];
                        }
                        if self.ci[i][2] == 0 {
                            self.ci[i][2] = self.wi[i];
                        }
                        if (k == nw && nn(k, 1, self) == 0 && nn(k, 2, self) == 0) || (i == n && k < nw) {
                            at = At::L1270;
                            continue;
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Loads (lumped, S-domain and conductor loss).
    fn set_loads(&mut self, ap: &AntDef, cfg: &EngineConfig) -> Result<(), CalcError> {
        let n = self.n as i32;
        if ap.loads_enabled {
            for l in &ap.loads {
                if l.plus_no > n {
                    return Err(CalcError::PulseOutOfRange { what: "load", name: l.plus.clone(), max: self.n });
                }
                let mut d = LoadDef { pulse: l.plus_no.max(0) as usize, ls: 0, la: [[0.0; SMAX + 1]; 3] };
                match l.ty {
                    0 => {
                        let (lu, cp, q) = (l.a[0], l.b[0], l.a[1]);
                        if lu != 0.0 && cp != 0.0 {
                            // parallel resonant circuit
                            let (r, x) = calc_para_z(ap.cfq, lu, cp, q);
                            d.la[1][1] = r;
                            d.la[2][1] = x;
                        } else if lu != 0.0 {
                            d.la[2][1] = ap.cfq * PAI2 * lu;
                            d.la[1][1] = if q != 0.0 { d.la[2][1] / q } else { 0.0 };
                        } else {
                            d.la[2][1] = -1.0 / (ap.cfq * PAI2 * cp * 1.0e-6);
                            d.la[1][1] = if q != 0.0 { -(d.la[2][1] / q) } else { 0.0 };
                        }
                    }
                    1 => {
                        d.la[1][1] = l.a[0];
                        d.la[2][1] = l.b[0];
                    }
                    _ => {
                        d.ls = l.sn;
                        for jj in 0..d.ls.max(0) as usize {
                            d.la[1][jj] = l.a[jj];
                            d.la[2][jj] = l.b[jj];
                        }
                        if d.ls <= 0 {
                            d.ls = 1;
                            d.la[1][1] = 0.0;
                            d.la[2][1] = 0.0;
                        }
                    }
                }
                self.loads.push(d);
            }
        }
        if cfg.wire_material != WireMaterial::Lossless {
            for wp in &ap.wires {
                if wp.r != 0.0 {
                    let mut d =
                        LoadDef { pulse: (wp.pno + wp.pmax / 2).max(0) as usize, ls: 0, la: [[0.0; SMAX + 1]; 3] };
                    d.la[1][1] = wire_loss(ap, wp, cfg.wire_material);
                    self.loads.push(d);
                }
            }
        }
        Ok(())
    }

    fn set_sources(&mut self, ap: &AntDef) -> Result<(), CalcError> {
        if ap.sources.is_empty() {
            return Err(CalcError::NoSources);
        }
        for c in &ap.sources {
            if c.plus_no > self.n as i32 || c.plus_no < 1 {
                return Err(CalcError::PulseOutOfRange { what: "source", name: c.plus.clone(), max: self.n });
            }
            self.sources.push(SourceDef {
                pulse: c.plus_no as usize,
                vr: c.ev * (c.deg * self.p0).cos(),
                vi: c.ev * (c.deg * self.p0).sin(),
            });
        }
        Ok(())
    }
}
