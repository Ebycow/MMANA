//! Kernel integration: PSI(P1, P2, P3) = T1 + j·T2 (L135 / S28) and the entry
//! points that place the observation and source points (S56, S66, S75, S87,
//! S102, L113).
//!
//! `fvs` is the original's "vector or scalar" flag (0 vector potential,
//! 1 scalar potential, -1 near field); `k` is +1 for the real source and -1 for
//! its ground image; `obs`/`src` are the observation/source pulses (`I`, `J`),
//! needed for the same-wire test that selects the exact kernel.

use super::{Mininec, int_};

/// How far past 1.1, 6 or 10 the distance ratio must be before the exact
/// kernel or the full order of integration is given up. The original
/// compares exactly; on a regular grid the ratio lands on a threshold, and
/// the rounding of the coordinates chose the side: one coordinate an ulp off
/// moved LAZY-H's |Z| by 0.03 %, and a trial the optimiser listed came out
/// 0.47 % apart when its values were written back (a quarter-wave vertical
/// fed through a 5 cm stub). Within the margin the more accurate side is
/// taken, as the oracle does (reference/oracle/extract.sh).
const ORDER_MARGIN: f64 = 1.0 + 1e-9;

/// Source-to-observation vectors handed to the integrator.
#[derive(Clone, Copy)]
struct Span {
    /// S(U) - S(M)
    x2: f64,
    y2: f64,
    z2: f64,
    /// S(V) - S(M)
    v1: f64,
    v2: f64,
    v3: f64,
}

impl Mininec {
    /// S28: adds the integrand at parameter `t` to (`t3`, `t4`).
    #[allow(clippy::too_many_arguments)]
    fn s28(&self, sp: &Span, t: f64, k: i32, p4: usize, a2: f64, i6f: f64, t3: &mut f64, t4: &mut f64) {
        let (x3, y3, z3) = if k < 0 {
            (sp.v1 + t * (sp.x2 - sp.v1), sp.v2 + t * (sp.y2 - sp.v2), sp.v3 + t * (sp.z2 - sp.v3))
        } else {
            (sp.x2 + t * (sp.v1 - sp.x2), sp.y2 + t * (sp.v2 - sp.y2), sp.z2 + t * (sp.v3 - sp.z2))
        };
        let d3 = x3 * x3 + y3 * y3 + z3 * z3;
        let a = self.a[p4];
        let d;
        // mod for small radius to wavelength ratio
        if a <= self.srm {
            d = d3.sqrt();
        } else {
            let mut dd = d3 + a2;
            if dd > 0.0 {
                dd = dd.sqrt();
            }
            d = dd;
            // criteria for using reduced kernel
            if i6f != 0.0 {
                // exact kernel calculation with elliptic integral
                let c = &self.c;
                let b = d3 / (d3 + 4.0 * a2);
                let w0 = c[0] + b * (c[1] + b * (c[2] + b * (c[3] + b * c[4])));
                let w1 = c[5] + b * (c[6] + b * (c[7] + b * (c[8] + b * c[9])));
                let v0 = (w0 - w1 * b.ln()) * (1.0 - b).sqrt();
                *t3 = *t3 + (v0 + (d3 / (64.0 * a2)).ln() / 2.0) / self.p / a - 1.0 / d;
            }
        }
        let b1 = d * self.w;
        // exp(-j k r) / r
        *t3 = *t3 + b1.cos() / d;
        *t4 = *t4 - b1.sin() / d;
    }

    /// L135: Gaussian quadrature of the kernel over source segment `p4`.
    #[allow(clippy::too_many_arguments)]
    fn l135(&self, sp: Span, p2: f64, p3: f64, p4: usize, fvs: i32, k: i32, obs: usize, src: usize) -> (f64, f64) {
        let mut d0 = sp.x2 * sp.x2 + sp.y2 * sp.y2 + sp.z2 * sp.z2;
        if d0 > 0.0 {
            d0 = d0.sqrt();
        }
        let mut d3 = sp.v1 * sp.v1 + sp.v2 * sp.v2 + sp.v3 * sp.v3;
        if d3 > 0.0 {
            d3 = d3.sqrt();
        }
        let a = self.a[p4];
        let seg = self.s[p4];
        let a2 = a * a;
        let s4 = (p3 - p2) * seg;
        let mut t1 = 0.0;
        let mut t2 = 0.0;
        let mut i6f = 0.0;
        let mut f2 = 1.0;
        let mut l = 7usize;
        let t = (d0 + d3) / seg;

        // criteria for exact kernel
        let mut skip_order_reduction = false;
        if !(t > 1.1 * ORDER_MARGIN) && fvs != -1 {
            let i = self.wi[obs] as usize;
            let j = self.wi[src] as usize;
            let (ji, jj) = (self.j2[i], self.j2[j]);
            if ji[1] == jj[1] || ji[1] == jj[2] || ji[2] == jj[1] || ji[2] == jj[2] {
                if a > self.srm {
                    f2 = 2.0 * (p3 - p2);
                    i6f = (1.0 - (s4 / f2 / 8.0 / a).ln()) / self.p / a;
                    skip_order_reduction = true;
                } else if fvs == 1 {
                    return (2.0 * (seg / a).ln(), -self.w * seg);
                } else {
                    return ((seg / a).ln(), -self.w * seg / 2.0);
                }
            }
        }
        if !skip_order_reduction {
            if t > 6.0 * ORDER_MARGIN {
                l = 3;
            }
            if t > 10.0 * ORDER_MARGIN {
                l = 1;
            }
        }
        let i5 = l + l;
        loop {
            let mut t3 = 0.0;
            let mut t4 = 0.0;
            self.s28(&sp, (self.q[l] + 0.5) / f2, k, p4, a2, i6f, &mut t3, &mut t4);
            self.s28(&sp, (0.5 - self.q[l]) / f2, k, p4, a2, i6f, &mut t3, &mut t4);
            l += 1;
            t1 = t1 + self.q[l] * t3;
            t2 = t2 + self.q[l] * t4;
            l += 1;
            if l >= i5 {
                break;
            }
        }
        (s4 * (t1 + i6f), s4 * t2)
    }

    /// Point `p` (integer = break point, half-integer = segment midpoint),
    /// with the z coordinate multiplied by `k` for the image.
    fn point(&self, p: f64, k: f64) -> (f64, f64, f64) {
        let i4 = int_(p);
        if i4 as f64 == p {
            (self.x[i4], self.y[i4], k * self.z[i4])
        } else {
            let i5 = i4 + 1;
            ((self.x[i4] + self.x[i5]) / 2.0, (self.y[i4] + self.y[i5]) / 2.0, k * (self.z[i4] + self.z[i5]) / 2.0)
        }
    }

    /// L113: S(U)-S(M) and S(V)-S(M) for observation point (x1, y1, z1).
    #[allow(clippy::too_many_arguments)]
    fn l113(
        &self,
        x1: f64,
        y1: f64,
        z1: f64,
        p2: f64,
        p3: f64,
        p4: usize,
        fvs: i32,
        k: i32,
        obs: usize,
        src: usize,
    ) -> (f64, f64) {
        let kf = k as f64;
        let (ux, uy, uz) = self.point(p2, kf);
        let (vx, vy, vz) = self.point(p3, kf);
        let sp = Span { x2: ux - x1, y2: uy - y1, z2: uz - z1, v1: vx - x1, v2: vy - y1, v3: vz - z1 };
        self.l135(sp, p2, p3, p4, fvs, k, obs, src)
    }

    /// S87: PSI for the scalar potential, observation at a segment midpoint.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn s87(&self, p1: f64, p2: f64, p3: f64, p4: usize, k: i32, obs: usize, src: usize) -> (f64, f64) {
        if k >= 1 && self.a[p4] <= self.srm && p3 == p2 + 1.0 && p1 == (p2 + p3) / 2.0 {
            return (2.0 * (self.s[p4] / self.a[p4]).ln(), -self.w * self.s[p4]);
        }
        let i4 = int_(p1);
        let i5 = i4 + 1;
        let x1 = (self.x[i4] + self.x[i5]) / 2.0;
        let y1 = (self.y[i4] + self.y[i5]) / 2.0;
        let z1 = (self.z[i4] + self.z[i5]) / 2.0;
        self.l113(x1, y1, z1, p2, p3, p4, 1, k, obs, src)
    }

    /// S102: PSI for the vector potential, observation at a break point.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn s102(&self, p1: f64, p2: f64, p3: f64, p4: usize, k: i32, obs: usize, src: usize) -> (f64, f64) {
        if k >= 1 && self.a[p4] < self.srm && obs == src && p3 == p2 + 0.5 {
            return ((self.s[p4] / self.a[p4]).ln(), -self.w * self.s[p4] / 2.0);
        }
        let p1i = p1 as usize;
        self.l113(self.x[p1i], self.y[p1i], self.z[p1i], p2, p3, p4, 0, k, obs, src)
    }

    /// S56: near-field PSI with the observation point offset along (t5, t6, t7).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn s56(
        &self,
        o: (f64, f64, f64),
        t: (f64, f64, f64),
        p1: f64,
        p2: f64,
        p3: f64,
        p4: usize,
        k: i32,
    ) -> (f64, f64) {
        let x1 = o.0 + p1 * t.0 / 2.0;
        let y1 = o.1 + p1 * t.1 / 2.0;
        let z1 = o.2 + p1 * t.2 / 2.0;
        let kf = k as f64;
        let p2i = p2 as usize;
        let p3i = p3 as usize;
        let sp = Span {
            x2: x1 - self.x[p2i],
            y2: y1 - self.y[p2i],
            z2: z1 - kf * self.z[p2i],
            v1: x1 - self.x[p3i],
            v2: y1 - self.y[p3i],
            v3: z1 - kf * self.z[p3i],
        };
        self.l135(sp, p2, p3, p4, -1, k, 0, 0)
    }

    /// S66: near-field PSI(0, J-1/2, J).
    pub(super) fn s66(&self, o: (f64, f64, f64), p2: f64, p3: f64, p4: usize, k: i32) -> (f64, f64) {
        let kf = k as f64;
        let i4 = int_(p2);
        let i5 = i4 + 1;
        let p3i = p3 as usize;
        let sp = Span {
            x2: o.0 - (self.x[i4] + self.x[i5]) / 2.0,
            y2: o.1 - (self.y[i4] + self.y[i5]) / 2.0,
            z2: o.2 - kf * (self.z[i4] + self.z[i5]) / 2.0,
            v1: o.0 - self.x[p3i],
            v2: o.1 - self.y[p3i],
            v3: o.2 - kf * self.z[p3i],
        };
        self.l135(sp, p2, p3, p4, -1, k, 0, 0)
    }

    /// S75: near-field PSI(0, J, J+1/2).
    pub(super) fn s75(&self, o: (f64, f64, f64), p2: f64, p3: f64, p4: usize, k: i32) -> (f64, f64) {
        let kf = k as f64;
        let p2i = p2 as usize;
        let i4 = int_(p3);
        let i5 = i4 + 1;
        let sp = Span {
            x2: o.0 - self.x[p2i],
            y2: o.1 - self.y[p2i],
            z2: o.2 - kf * self.z[p2i],
            v1: o.0 - (self.x[i4] + self.x[i5]) / 2.0,
            v2: o.1 - (self.y[i4] + self.y[i5]) / 2.0,
            v3: o.2 - kf * (self.z[i4] + self.z[i5]) / 2.0,
        };
        self.l135(sp, p2, p3, p4, -1, k, 0, 0)
    }
}
