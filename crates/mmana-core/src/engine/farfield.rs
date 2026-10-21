//! S621: far-field pattern (dBi), with perfect or real (reflection
//! coefficient) ground.

use super::{Mininec, sgn};
use serde::Serialize;

/// One direction of a far-field scan. Angles in degrees: `zenith` 0 = up,
/// 90 = horizon; `azimuth` 0 = +X.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FarFieldSample {
    pub zenith: f64,
    pub azimuth: f64,
    /// Vertically polarised gain (dBi), -999 when below 1e-30 and below the
    /// horizon of a ground.
    pub vertical_db: f64,
    pub horizontal_db: f64,
    pub total_db: f64,
}

/// S184: complex square root W6 + jW7 = sqrt(Z6 + jZ7).
fn csqrt(z6: f64, z7: f64) -> (f64, f64) {
    let t6 = ((z6.abs() + (z6 * z6 + z7 * z7).sqrt()) / 2.0).sqrt();
    let t7 = z7.abs() / 2.0 / t6;
    if z6 < 0.0 { (t7, if z7 < 0.0 { -t6 } else { t6 }) } else { (t6, if z7 < 0.0 { -t7 } else { t7 }) }
}

/// x² by repeated multiplication (`POW(x, 2)`).
#[inline]
fn sq(x: f64) -> f64 {
    x * x
}

impl Mininec {
    /// Scans `nz` zenith angles from `za` in steps of `zc` for each of `na`
    /// azimuths from `aa` in steps of `ac` (a count of 0 means 1). Angles
    /// accumulate by repeated addition, as in the original. Directions are
    /// evaluated in parallel and handed to `sink` in scan order.
    #[allow(clippy::too_many_arguments)]
    pub fn far_field(
        &self,
        za: f64,
        zc: f64,
        nz: i32,
        aa: f64,
        ac: f64,
        na: i32,
        mut sink: impl FnMut(FarFieldSample),
    ) {
        let p = self.p;
        // impedance = 1/sqrt(dielectric constant) for each medium
        let mut mz1 = vec![0.0; self.nm + 1];
        let mut mz2 = vec![0.0; self.nm + 1];
        for i in 1..=self.nm {
            let z6 = self.med_t[i];
            let z7 = -self.med_v[i] / (2.0 * p * self.f * 8.85E-06);
            let (w6, w7) = csqrt(z6, z7);
            let d = w6 * w6 + w7 * w7;
            mz1[i] = w6 / d;
            mz2[i] = -w7 / d;
        }
        let nz = if nz == 0 { 1 } else { nz };
        let na = if na == 0 { 1 } else { na };
        let mut dirs = Vec::with_capacity(nz.max(0) as usize * na.max(0) as usize);
        let mut q1 = aa;
        for _ in 1..=na {
            let mut q2 = za;
            for _ in 1..=nz {
                dirs.push((q1, q2));
                q2 += zc;
            }
            q1 += ac;
        }
        let samples = crate::par::map(dirs.len(), |d| self.far_field_at(dirs[d].0, dirs[d].1, &mz1, &mz2));
        samples.into_iter().for_each(&mut sink);
    }

    /// One direction of the S621 scan: azimuth `q1`, zenith `q2` (degrees).
    fn far_field_at(&self, q1: f64, q2: f64, mz1: &[f64], mz2: &[f64]) -> FarFieldSample {
        let p0 = self.p0;
        let k9f = 0.016678 / self.pwr;
        let u3 = q1 * p0;
        let v1 = -u3.sin();
        let v2 = u3.cos();
        let u4 = q2 * p0;
        let r3 = u4.cos();
        // Below the horizon of a ground there is no far field; the original
        // computes one there anyway, from the image. Zenith angles past 270°
        // (the back of MMANA's own vertical cut) are above it again, and
        // 1e-12 keeps a horizon reached by adding steps.
        if self.g == -1 && r3 < -1e-12 {
            return FarFieldSample {
                zenith: q2,
                azimuth: q1,
                vertical_db: -999.0,
                horizontal_db: -999.0,
                total_db: -999.0,
            };
        }
        let t3 = -u4.sin();
        let t1 = r3 * v2;
        let t2 = -r3 * v1;
        let r1 = -t3 * v2;
        let r2 = t3 * v1;
        let (mut x1, mut y1, mut z1, mut x2, mut y2, mut z2) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let mut k = 1i32;
        while k >= self.g {
            let kf = k as f64;
            for i in 1..=self.n {
                let [_, ci1, ci2] = self.ci[i];
                let grounded = ci1 == -ci2;
                if k <= 0 && grounded {
                    continue;
                }
                let j = (2 * self.wi[i] - 1 + i as i32) as usize;
                if k == 1 || self.nm == 0 {
                    // standard case; the phase depends only on the break point,
                    // so one evaluation serves both ends of the pulse
                    let s2 = self.w * (self.x[j] * r1 + self.y[j] * r2 + self.z[j] * kf * r3);
                    let (s1, s2) = (s2.cos(), s2.sin());
                    // for each end of pulse compute a contribution to E-field
                    for c in [ci1, ci2] {
                        let l = c.unsigned_abs() as usize;
                        let f3 = sgn(c) as f64 * self.w * self.s[l] / 2.0;
                        if grounded && f3 < 0.0 {
                            continue;
                        }
                        let b1 = f3 * (s1 * self.cr[i] - s2 * self.cim[i]);
                        let b2 = f3 * (s1 * self.cim[i] + s2 * self.cr[i]);
                        if grounded {
                            z1 = z1 + 2.0 * b1 * self.cg[l];
                            z2 = z2 + 2.0 * b2 * self.cg[l];
                        } else {
                            x1 = x1 + kf * b1 * self.ca[l];
                            x2 = x2 + kf * b2 * self.ca[l];
                            y1 = y1 + kf * b1 * self.cb[l];
                            y2 = y2 + kf * b2 * self.cb[l];
                            z1 = z1 + b1 * self.cg[l];
                            z2 = z2 + b2 * self.cg[l];
                        }
                    }
                    continue;
                }
                // real ground case: find the specular distance
                let mut t4 = 100000.0;
                if r3 != 0.0 {
                    t4 = -self.z[j] * t3 / r3;
                }
                let mut b9 = t4 * v2 + self.x[j];
                if self.tb != 1 {
                    b9 = b9 * b9 + sq(self.y[j] - t4 * v1);
                    if b9 > 0.0 {
                        b9 = b9.sqrt();
                    }
                }
                // search for the corresponding medium
                let mut jm = self.nm;
                for j1 in (1..=self.nm).rev() {
                    if b9 <= self.med_u[j1] {
                        jm = j1;
                    }
                }
                // impedance at specular point
                let mut z4 = mz1[jm];
                let mut z5 = mz2[jm];
                // ground screen impedance in parallel
                if self.nr != 0 && !(b9 > self.med_u[1]) {
                    let nr = self.nr as f64;
                    let r = b9 + nr * self.rr;
                    let z8 = self.w * r * (r / (nr * self.rr)).ln() / nr;
                    let s8 = -z5 * z8;
                    let s9 = z4 * z8;
                    let t8 = z4;
                    let t9 = z5 + z8;
                    let d = t8 * t8 + t9 * t9;
                    z4 = (s8 * t8 + s9 * t9) / d;
                    z5 = (s9 * t8 - s8 * t9) / d;
                }
                // sqrt(1 - Z^2 sin^2)
                let z6 = 1.0 - (z4 * z4 - z5 * z5) * t3 * t3;
                let z7 = -(2.0 * z4 * z5) * t3 * t3;
                let (w6, w7) = csqrt(z6, z7);
                // vertical reflection coefficient
                let s8 = r3 - (w6 * z4 - w7 * z5);
                let s9 = -(w6 * z5 + w7 * z4);
                let t8 = r3 + (w6 * z4 - w7 * z5);
                let t9 = w6 * z5 + w7 * z4;
                let d = t8 * t8 + t9 * t9;
                let v8 = (s8 * t8 + s9 * t9) / d;
                let v9 = (s9 * t8 - s8 * t9) / d;
                // horizontal reflection coefficient
                let s8 = w6 - r3 * z4;
                let s9 = w7 - r3 * z5;
                let t8 = w6 + r3 * z4;
                let t9 = w7 + r3 * z5;
                let d = t8 * t8 + t9 * t9;
                let h8 = (s8 * t8 + s9 * t9) / d - v8;
                let h9 = (s9 * t8 - s8 * t9) / d - v9;
                let s2 = self.w * (self.x[j] * r1 + self.y[j] * r2 - (self.z[j] - 2.0 * self.med_h[jm]) * r3);
                let (s1, s2) = (s2.cos(), s2.sin());
                for c in [ci1, ci2] {
                    let l = c.unsigned_abs() as usize;
                    let f3 = sgn(c) as f64 * self.w * self.s[l] / 2.0;
                    if grounded && f3 < 0.0 {
                        continue;
                    }
                    // contribution to sum
                    let b1 = f3 * (s1 * self.cr[i] - s2 * self.cim[i]);
                    let b2 = f3 * (s1 * self.cim[i] + s2 * self.cr[i]);
                    let w6 = b1 * v8 - b2 * v9;
                    let w7 = b1 * v9 + b2 * v8;
                    let d = self.ca[l] * v1 + self.cb[l] * v2;
                    let z6 = d * (b1 * h8 - b2 * h9);
                    let z7 = d * (b1 * h9 + b2 * h8);
                    x1 = x1 - (self.ca[l] * w6 + v1 * z6);
                    x2 = x2 - (self.ca[l] * w7 + v1 * z7);
                    y1 = y1 - (self.cb[l] * w6 + v2 * z6);
                    y2 = y2 - (self.cb[l] * w7 + v2 * z7);
                    z1 = z1 + self.cg[l] * w6;
                    z2 = z2 + self.cg[l] * w7;
                }
            }
            k -= 2;
        }
        let h2 = -(x1 * t1 + y1 * t2 + z1 * t3) * self.g0;
        let h1 = (x2 * t1 + y2 * t2 + z2 * t3) * self.g0;
        let x4 = -(x1 * v1 + y1 * v2) * self.g0;
        let x3 = (x2 * v1 + y2 * v2) * self.g0;
        // pattern in dB
        let (mut pv, mut ph, mut pt) = (-999.0, -999.0, -999.0);
        let e1 = k9f * (h1 * h1 + h2 * h2);
        let e2 = k9f * (x3 * x3 + x4 * x4);
        let e3 = e1 + e2;
        if e1 > 1E-30 {
            pv = 4.343 * e1.ln();
        }
        if e2 > 1E-30 {
            ph = 4.343 * e2.ln();
        }
        if e3 > 1E-30 {
            pt = 4.343 * e3.ln();
        }
        FarFieldSample { zenith: q2, azimuth: q1, vertical_db: pv, horizontal_db: ph, total_db: pt }
    }
}
