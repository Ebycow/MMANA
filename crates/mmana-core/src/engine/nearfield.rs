//! S875: near electric / magnetic field on a grid of points.

use super::{Mininec, sgn};
use serde::{Deserialize, Serialize};

/// 1/√2: fields are reported as RMS values.
const IVSQ2: f64 = std::f64::consts::FRAC_1_SQRT_2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum NearFieldKind {
    Electric,
    Magnetic,
}

/// Start, step and count along one axis (metres). A step of 0 means one point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AxisRange {
    pub start: f64,
    pub step: f64,
    pub count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct NearFieldSpec {
    pub kind: NearFieldKind,
    /// Radiated power the fields are scaled to (W).
    pub power_w: f64,
    pub x: AxisRange,
    pub y: AxisRange,
    pub z: AxisRange,
}

/// One vector component at one point. Units: V/m (electric) or A/m (magnetic), RMS.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct NearFieldSample {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub component: char,
    pub real: f64,
    pub imag: f64,
    pub magnitude: f64,
    pub phase_deg: f64,
}

impl Mininec {
    pub fn near_field(&self, spec: &NearFieldSpec) -> Vec<NearFieldSample> {
        let magnetic = spec.kind == NearFieldKind::Magnetic;
        let axis = |a: &AxisRange| if a.step == 0.0 || a.count == 0 { 1 } else { a.count };
        let (nx, ny, nz) = (axis(&spec.x), axis(&spec.y), axis(&spec.z));
        // ratio of power levels
        let mut f1 = (spec.power_w / self.pwr).sqrt();
        if magnetic {
            f1 = f1 / self.s0 / 4.0 / self.p;
        }
        let mut out = Vec::new();
        for iz in 1..=nz {
            let zz = spec.z.start + (iz - 1) as f64 * spec.z.step;
            for iy in 1..=ny {
                let yy = spec.y.start + (iy - 1) as f64 * spec.y.step;
                for ix in 1..=nx {
                    let xx = spec.x.start + (ix - 1) as f64 * spec.x.step;
                    if magnetic {
                        self.magnetic_point(xx, yy, zz, f1, &mut out);
                    } else {
                        self.electric_point(xx, yy, zz, f1, &mut out);
                    }
                }
            }
        }
        out
    }

    /// Vector-potential terms (X3..Z5) of source pulse `j` for observation `o`.
    fn vector_terms(&self, o: (f64, f64, f64), j: usize, k: i32, f: (f64, f64, f64, f64)) -> ([f64; 6], f64) {
        let (f4, f5, f6, f7) = f;
        let [_, cj1, cj2] = self.ci[j];
        let (j1, j2) = (cj1.unsigned_abs() as usize, cj2.unsigned_abs() as usize);
        let j3 = if j1 > j2 { j1 } else { j2 };
        let kf = k as f64;
        // PSI(0, J, J+1/2)
        let p2 = (2 * j3 + j - 1) as f64;
        let p3 = p2 + 0.5;
        let (t1, t2) = self.s75(o, p2, p3, j2, k);
        let u1 = t1 * f5;
        let u2 = t2 * f5;
        // PSI(0, J-1/2, J)
        let (t1, t2) = self.s66(o, p2 - 0.5, p2, j1, k);
        let v1 = f4 * t1;
        let v2 = f4 * t2;
        let x3 = u1 * self.ca[j2] + v1 * self.ca[j1];
        let y3 = u1 * self.cb[j2] + v1 * self.cb[j1];
        let z3 = (f7 * u1 * self.cg[j2] + f6 * v1 * self.cg[j1]) * kf;
        let x5 = u2 * self.ca[j2] + v2 * self.ca[j1];
        let y5 = u2 * self.cb[j2] + v2 * self.cb[j1];
        let z5 = (f7 * u2 * self.cg[j2] + f6 * v2 * self.cg[j1]) * kf;
        ([x3, y3, z3, x5, y5, z5], p2)
    }

    fn electric_point(&self, xx: f64, yy: f64, zz: f64, f1: f64, out: &mut Vec<NearFieldSample>) {
        let o = (xx, yy, zz);
        for comp in 1..=3 {
            let t = match comp {
                1 => (2.0 * self.s0, 0.0, 0.0),
                2 => (0.0, 2.0 * self.s0, 0.0),
                _ => (0.0, 0.0, 2.0 * self.s0),
            };
            let (mut u7, mut u8) = (0.0, 0.0);
            for j in 1..=self.n {
                let [_, cj1, cj2] = self.ci[j];
                let (j1, j2) = (cj1.unsigned_abs() as usize, cj2.unsigned_abs() as usize);
                let (f4, f5) = (sgn(cj1) as f64, sgn(cj2) as f64);
                let (mut f6, mut f7) = (1.0, 1.0);
                let (mut u5, mut u6) = (0.0, 0.0);
                let mut k = 1i32;
                while k >= self.g {
                    if cj1 == -cj2 {
                        if k < 0 {
                            k -= 2;
                            continue;
                        }
                        f6 = f4;
                        f7 = f5;
                    }
                    let kf = k as f64;
                    let ([x3, y3, z3, x5, y5, z5], p2) = self.vector_terms(o, j, k, (f4, f5, f6, f7));
                    let d1 = (x3 * t.0 + y3 * t.1 + z3 * t.2) * self.w2;
                    let d2 = (x5 * t.0 + y5 * t.1 + z5 * t.2) * self.w2;
                    // PSI(.5, J, J+1) and PSI(-.5, J, J+1)
                    let (p2, p3) = (p2, p2 + 1.0);
                    let (a1, a2) = self.s56(o, t, 0.5, p2, p3, j2, k);
                    let (b1, b2) = self.s56(o, t, -0.5, p2, p3, j2, k);
                    let u1 = (b1 - a1) / self.s[j2];
                    let u2 = (b2 - a2) / self.s[j2];
                    // PSI(.5, J-1, J) and PSI(-.5, J-1, J)
                    let (p3, p2) = (p2, p2 - 1.0);
                    let (u3, u4) = self.s56(o, t, 0.5, p2, p3, j1, k);
                    let (t1, t2) = self.s56(o, t, -0.5, p2, p3, j1, k);
                    // gradient of scalar potential
                    u5 = (u1 + (u3 - t1) / self.s[j1] + d1) * kf + u5;
                    u6 = (u2 + (u4 - t2) / self.s[j1] + d2) * kf + u6;
                    k -= 2;
                }
                u7 = u5 * self.cr[j] - u6 * self.cim[j] + u7;
                u8 = u6 * self.cr[j] + u5 * self.cim[j] + u8;
            }
            // imaginary / real part of the electric field
            let u7 = -self.m * u7 / self.s0;
            let u8 = self.m * u8 / self.s0;
            // (the original takes 0 when both are 0, which the root gives too)
            let s1 = (u7 * u7 + u8 * u8).sqrt();
            let mut s2 = 0.0;
            if u8 != 0.0 {
                s2 = (u7 / u8).atan() / self.p0;
            }
            if !(u8 > 0.0) {
                s2 += sgn_f(u7) * 180.0;
            }
            out.push(NearFieldSample {
                x: xx,
                y: yy,
                z: zz,
                component: ['X', 'Y', 'Z'][comp - 1],
                real: f1 * u8 * IVSQ2,
                imag: f1 * u7 * IVSQ2,
                magnitude: f1 * s1 * IVSQ2,
                phase_deg: s2,
            });
        }
    }

    fn magnetic_point(&self, xx: f64, yy: f64, zz: f64, f1: f64, out: &mut Vec<NearFieldSample>) {
        let mut h = [0.0f64; 7];
        for comp in 1..=3 {
            // aKf[1..=6][1..=2]: vector potential at the two offset points
            let mut kf_ = [[0.0f64; 3]; 7];
            for (j9, j8) in [(1usize, -1.0f64), (2, 1.0)] {
                let mut o = (xx, yy, zz);
                match comp {
                    1 => o.0 = xx + j8 * self.s0 / 2.0,
                    2 => o.1 = yy + j8 * self.s0 / 2.0,
                    _ => o.2 = zz + j8 * self.s0 / 2.0,
                }
                for j in 1..=self.n {
                    let [_, cj1, cj2] = self.ci[j];
                    let (f4, f5) = (sgn(cj1) as f64, sgn(cj2) as f64);
                    let (mut f6, mut f7) = (1.0, 1.0);
                    let mut k = 1i32;
                    while k >= self.g {
                        if cj1 == -cj2 {
                            if k < 0 {
                                k -= 2;
                                continue;
                            }
                            f6 = f4;
                            f7 = f5;
                        }
                        let kf = k as f64;
                        let ([x3, y3, z3, x5, y5, z5], _) = self.vector_terms(o, j, k, (f4, f5, f6, f7));
                        let (cr, ci) = (self.cr[j], self.cim[j]);
                        kf_[1][j9] = kf_[1][j9] + (x3 * cr - x5 * ci) * kf;
                        kf_[2][j9] = kf_[2][j9] + (x5 * cr + x3 * ci) * kf;
                        kf_[3][j9] = kf_[3][j9] + (y3 * cr - y5 * ci) * kf;
                        kf_[4][j9] = kf_[4][j9] + (y5 * cr + y3 * ci) * kf;
                        kf_[5][j9] = kf_[5][j9] + (z3 * cr - z5 * ci) * kf;
                        kf_[6][j9] = kf_[6][j9] + (z5 * cr + z3 * ci) * kf;
                        k -= 2;
                    }
                }
            }
            // differences of vector potential A (curl)
            match comp {
                1 => {
                    h[3] = kf_[5][1] - kf_[5][2];
                    h[4] = kf_[6][1] - kf_[6][2];
                    h[5] = kf_[3][2] - kf_[3][1];
                    h[6] = kf_[4][2] - kf_[4][1];
                }
                2 => {
                    h[1] = kf_[5][2] - kf_[5][1];
                    h[2] = kf_[6][2] - kf_[6][1];
                    h[5] = h[5] - kf_[1][2] + kf_[1][1];
                    h[6] = h[6] - kf_[2][2] + kf_[2][1];
                }
                _ => {
                    h[1] = h[1] - kf_[3][2] + kf_[3][1];
                    h[2] = h[2] - kf_[4][2] + kf_[4][1];
                    h[3] = h[3] + kf_[1][2] - kf_[1][1];
                    h[4] = h[4] + kf_[2][2] - kf_[2][1];
                }
            }
        }
        for (i, comp) in [(1usize, 'X'), (3, 'Y'), (5, 'Z')] {
            let s1 = (h[i] * h[i] + h[i + 1] * h[i + 1]).sqrt();
            let mut s2 = 0.0;
            if h[i] != 0.0 {
                s2 = (h[i + 1] / h[i]).atan() / self.p0;
            }
            if h[i] <= 0.0 {
                s2 += sgn_f(h[i + 1]) * 180.0;
            }
            out.push(NearFieldSample {
                x: xx,
                y: yy,
                z: zz,
                component: comp,
                real: f1 * h[i] * IVSQ2,
                imag: f1 * h[i + 1] * IVSQ2,
                magnitude: f1 * s1 * IVSQ2,
                phase_deg: s2,
            });
        }
    }
}

/// `SGN()` of a double, as an integer-valued double.
fn sgn_f(c: f64) -> f64 {
    if c < 0.0 {
        -1.0
    } else if c > 0.0 {
        1.0
    } else {
        0.0
    }
}
