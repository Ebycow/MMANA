//! S196: impedance matrix fill, loads, factorisation and solution.
//!
//! The matrix is stored as `f32` exactly like the original (`float *aZR`), and
//! every expression keeps the original's mix of single- and double-precision
//! arithmetic: products of two matrix elements are evaluated in `f32`, products
//! with a `double` are promoted. This is what makes the results bit-compatible.

use super::circuit::calc_swr;
use super::{Mininec, SourceSolution, sgn};

/// `POW`: integer power by repeated multiplication (not `powi`, whose
/// rounding differs).
fn pow_n(d: f64, n: i32) -> f64 {
    match n {
        0 => 1.0,
        n if n > 0 => {
            let mut dd = d;
            for _ in 1..n {
                dd *= d;
            }
            dd
        }
        n => {
            let mut dd = d;
            for _ in 1..-n {
                dd *= d;
            }
            1.0 / dd
        }
    }
}

impl Mininec {
    /// Runs S196 completely (fill, loads, factor, solve, source data).
    pub fn solve(&mut self) {
        self.fill();
        self.add_loads();
        let piv = self.factor();
        self.substitute(&piv);
        self.source_data();
    }

    #[inline]
    fn add_f64(m: &mut [f32], idx: usize, v: f64) {
        m[idx] = (m[idx] as f64 + v) as f32;
    }

    /// Impedance matrix fill (observation loop I, source loop J, image loop K).
    fn fill(&mut self) {
        let n = self.n;
        self.zr = vec![0.0; n * n];
        self.zi = vec![0.0; n * n];
        for i in 1..=n {
            let [_, ci1, ci2] = self.ci[i];
            let i1 = ci1.unsigned_abs() as usize;
            let i2 = ci2.unsigned_abs() as usize;
            let f4 = sgn(ci1) as f64 * self.s[i1];
            let f5 = sgn(ci2) as f64 * self.s[i2];
            // R(M + 1/2) - R(M - 1/2) has components (T5, T6, T7)
            let t5 = f4 * self.ca[i1] + f5 * self.ca[i2];
            let t6 = f4 * self.cb[i1] + f5 * self.cb[i2];
            let mut t7 = f4 * self.cg[i1] + f5 * self.cg[i2];
            if ci1 == -ci2 {
                t7 = self.s[i1] * (self.cg[i1] + self.cg[i2]);
            }
            for j in 1..=n {
                let [_, cj1, cj2] = self.ci[j];
                let j1 = cj1.unsigned_abs() as usize;
                let j2 = cj2.unsigned_abs() as usize;
                let f4 = sgn(cj1) as f64;
                let f5 = sgn(cj2) as f64;
                let mut f6 = 1.0;
                let mut f7 = 1.0;
                let mut k = 1i32;
                while k >= self.g {
                    self.fill_element(i, j, k, (i1, i2, j1, j2), (f4, f5, &mut f6, &mut f7), (t5, t6, t7));
                    k -= 2;
                }
            }
        }
    }

    /// Body of the image loop (L235-L332) for one (I, J, K).
    #[allow(clippy::type_complexity)]
    fn fill_element(
        &mut self,
        i: usize,
        j: usize,
        k: i32,
        (i1, i2, j1, j2): (usize, usize, usize, usize),
        (f4, f5, f6, f7): (f64, f64, &mut f64, &mut f64),
        (t5, t6, t7): (f64, f64, f64),
    ) {
        let n = self.n;
        let [_, ci1, ci2] = self.ci[i];
        let [_, cj1, cj2] = self.ci[j];
        if cj1 == -cj2 {
            if k < 0 {
                return;
            }
            *f6 = f4;
            *f7 = f5;
        }
        let mut f8 = 0;
        let ij = self.mi(i, j);
        let mut compute = true;
        if k >= 0 {
            // set flag to avoid redundant calculations
            if i1 == i2
                && ((self.ca[i1] + self.cb[i1]) == 0.0 || ci1 == ci2)
                && j1 == j2
                && ((self.ca[j1] + self.cb[j1]) == 0.0 || cj1 == cj2)
            {
                if i1 == j1 {
                    f8 = 1;
                }
                if i == j {
                    f8 = 2;
                }
            }
            if self.zr[ij] != 0.0 {
                compute = false;
            }
        }
        if compute {
            self.compute_element(i, j, k, (j1, j2), (f4, f5, *f6, *f7), f8, (t5, t6, t7));
        }

        // L317: avoid redundant calculations
        if j < i || f8 == 0 {
            return;
        }
        let ji = self.mi(j, i);
        self.zr[ji] = self.zr[ij];
        self.zi[ji] = self.zi[ij];
        // segments on same wire same distance apart have same Z
        let p1 = j + 1;
        if p1 > n {
            return;
        }
        let cp = self.ci[p1];
        if cp[1] != cp[2] {
            return;
        }
        if cp[2] != cj2 {
            if cp[2] != -cj2 {
                return;
            }
            if (self.ca[j2] + self.cb[j2]) != 0.0 {
                return;
            }
        }
        let p2 = i + 1;
        if p2 > n {
            return;
        }
        let pp = self.mi(p1, p2);
        self.zr[pp] = self.zr[ij];
        self.zi[pp] = self.zi[ij];
    }

    /// L248-L314: one matrix element contribution.
    #[allow(clippy::too_many_arguments)]
    fn compute_element(
        &mut self,
        i: usize,
        j: usize,
        k: i32,
        (j1, j2): (usize, usize),
        (f4, f5, f6, f7): (f64, f64, f64, f64),
        f8: i32,
        (t5, t6, t7): (f64, f64, f64),
    ) {
        let kf = k as f64;
        // compute PSI(M, N, N+1/2)
        let mut p1 = (2 * self.wi[i] + i as i32 - 1) as f64;
        let mut p2 = (2 * self.wi[j] + j as i32 - 1) as f64;
        let mut p3 = p2 + 0.5;
        let (mut t1, mut t2) = self.s102(p1, p2, p3, j2, k, i, j);
        let mut u1 = f5 * t1;
        let mut u2 = f5 * t2;
        // compute PSI(M, N-1/2, N)
        p3 = p2;
        p2 -= 0.5;
        if f8 < 2 {
            (t1, t2) = self.s102(p1, p2, p3, j1, k, i, j);
        }
        let v1 = f4 * t1;
        let v2 = f4 * t2;
        // S(N+1/2)*PSI(M,N,N+1/2) + S(N-1/2)*PSI(M,N-1/2,N)
        let x3 = u1 * self.ca[j2] + v1 * self.ca[j1];
        let y3 = u1 * self.cb[j2] + v1 * self.cb[j1];
        let z3 = (f7 * u1 * self.cg[j2] + f6 * v1 * self.cg[j1]) * kf;
        // real part of vector potential contribution
        let d1 = self.w2 * (x3 * t5 + y3 * t6 + z3 * t7);
        let x3 = u2 * self.ca[j2] + v2 * self.ca[j1];
        let y3 = u2 * self.cb[j2] + v2 * self.cb[j1];
        let z3 = (f7 * u2 * self.cg[j2] + f6 * v2 * self.cg[j1]) * kf;
        // imaginary part of vector potential contribution
        let d2 = self.w2 * (x3 * t5 + y3 * t6 + z3 * t7);
        // compute PSI(M+1/2, N, N+1)
        p1 += 0.5;
        if f8 == 2 {
            p1 -= 1.0;
        }
        p2 = p3;
        p3 += 1.0;
        let (u5, u6);
        'l314: {
            if f8 == 1 {
                u5 = f5 * u1 + t1;
                u6 = f5 * u2 + t2;
            } else {
                (t1, t2) = self.s87(p1, p2, p3, j2, k, i, j);
                if f8 >= 2 {
                    u1 = (2.0 * t1 - 4.0 * u1 * f5) / self.s[j1];
                    u2 = (2.0 * t2 - 4.0 * u2 * f5) / self.s[j1];
                    break 'l314;
                }
                u5 = t1;
                u6 = t2;
            }
            // compute PSI(M-1/2, N, N+1)  (L291)
            p1 -= 1.0;
            (t1, t2) = self.s87(p1, p2, p3, j2, k, i, j);
            u1 = (t1 - u5) / self.s[j2];
            u2 = (t2 - u6) / self.s[j2];
            // compute PSI(M+1/2, N-1, N)
            p1 += 1.0;
            p3 = p2;
            p2 -= 1.0;
            (t1, t2) = self.s87(p1, p2, p3, j1, k, i, j);
            let u3 = t1;
            let u4 = t2;
            // compute PSI(M-1/2, N-1, N)
            if f8 >= 1 {
                t1 = u5;
                t2 = u6;
            } else {
                p1 -= 1.0;
                (t1, t2) = self.s87(p1, p2, p3, j1, k, i, j);
            }
            // gradient of scalar potential contribution
            u1 += (u3 - t1) / self.s[j1];
            u2 += (u4 - t2) / self.s[j1];
        }
        // sum into impedance matrix
        let ij = self.mi(i, j);
        Self::add_f64(&mut self.zr, ij, kf * (d1 + u1));
        Self::add_f64(&mut self.zi, ij, kf * (d2 + u2));
    }

    /// Addition of loads (L343-L374).
    fn add_loads(&mut self) {
        // The original reads the image-loop variable K after the fill loop,
        // where it is always negative.
        let k_after_fill = -1;
        let f5 = 2.0 * self.p * self.f * 1000000.0;
        for l in 0..self.loads.len() {
            let ld = self.loads[l];
            let (lr, li);
            if ld.ls >= 1 {
                // S-domain loads
                let (mut u1, mut u2, mut d1, mut d2) = (0.0, 0.0, 0.0, 0.0);
                let mut s = -1.0;
                let mut jj = 0;
                while jj < ld.ls {
                    let j = jj as usize;
                    s = -s;
                    u1 = u1 + ld.la[1][j] * s * pow_n(f5, jj);
                    d1 = d1 + ld.la[2][j] * s * pow_n(f5, jj);
                    u2 = u2 + ld.la[1][j + 1] * s * pow_n(f5, jj + 1);
                    d2 = d2 + ld.la[2][j + 1] * s * pow_n(f5, jj + 1);
                    jj += 2;
                }
                let d = d1 * d1 + d2 * d2;
                li = (u2 * d1 - d2 * u1) / d;
                lr = (u1 * d1 + u2 * d2) / d;
            } else {
                lr = ld.la[1][1];
                li = ld.la[2][1];
            }
            let j = ld.pulse;
            if j == 0 {
                // unresolved pulse: the original writes outside the matrix
                continue;
            }
            let mut f2 = 1.0 / self.m;
            if self.ci[j][1] == -self.ci[j][2] && k_after_fill < 0 {
                f2 = 2.0 / self.m;
            }
            let jj = self.mi(j, j);
            Self::add_f64(&mut self.zr, jj, f2 * li);
            self.zi[jj] = (self.zi[jj] as f64 - f2 * lr) as f32;
        }
    }

    /// Gaussian elimination with partial pivoting (L377-L426).
    fn factor(&mut self) -> Vec<usize> {
        let n = self.n;
        let mut piv = vec![0usize; n + 1];
        for k in 1..n {
            // search for pivot
            let kk = self.mi(k, k);
            let mut t = (self.zr[kk] * self.zr[kk] + self.zi[kk] * self.zi[kk]) as f64;
            let mut i1 = k;
            for i in k + 1..=n {
                let ik = self.mi(i, k);
                let t1 = (self.zr[ik] * self.zr[ik] + self.zi[ik] * self.zi[ik]) as f64;
                if t1 >= t {
                    i1 = i;
                    t = t1;
                }
            }
            // exchange rows K and I1
            if i1 != k {
                let (a, b) = (self.mi(k, 1), self.mi(i1, 1));
                for c in 0..n {
                    self.zr.swap(a + c, b + c);
                    self.zi.swap(a + c, b + c);
                }
            }
            piv[k] = i1;
            // subtract row K from rows K+1 to N
            for i in k + 1..=n {
                let ik = self.mi(i, k);
                let kk = self.mi(k, k);
                let (a, b, c, d) = (self.zr[ik], self.zi[ik], self.zr[kk], self.zi[kk]);
                let t1 = (a * c + b * d) as f64 / t;
                let t2 = (b * c - a * d) as f64 / t;
                self.zr[ik] = t1 as f32;
                self.zi[ik] = t2 as f32;
                let row_i = self.mi(i, 1);
                let row_k = self.mi(k, 1);
                for c in k..n {
                    let (ix, kx) = (row_i + c, row_k + c);
                    let (zrk, zik) = (self.zr[kx] as f64, self.zi[kx] as f64);
                    self.zr[ix] = (self.zr[ix] as f64 - (zrk * t1 - zik * t2)) as f32;
                    self.zi[ix] = (self.zi[ix] as f64 - (zrk * t2 + zik * t1)) as f32;
                }
            }
        }
        piv
    }

    /// Right-hand side, permutation, forward elimination and back
    /// substitution (L428-L475).
    fn substitute(&mut self, piv: &[usize]) {
        let n = self.n;
        self.cr = vec![0.0; n + 1];
        self.cim = vec![0.0; n + 1];
        for src in &self.sources {
            let e = src.pulse;
            let mut f2 = 1.0 / self.m;
            if self.ci[e][1] == -self.ci[e][2] {
                f2 = 2.0 / self.m;
            }
            self.cr[e] = f2 * src.vi;
            self.cim[e] = -f2 * src.vr;
        }
        // permute excitation
        for k in 1..n {
            let i1 = piv[k];
            if i1 != k {
                self.cr.swap(k, i1);
                self.cim.swap(k, i1);
            }
        }
        // forward elimination
        for i in 2..=n {
            let (mut t1, mut t2) = (0.0, 0.0);
            let row = self.mi(i, 1);
            for j in 1..i {
                let (zr, zi) = (self.zr[row + j - 1] as f64, self.zi[row + j - 1] as f64);
                t1 = t1 + zr * self.cr[j] - zi * self.cim[j];
                t2 = t2 + zr * self.cim[j] + zi * self.cr[j];
            }
            self.cr[i] -= t1;
            self.cim[i] -= t2;
        }
        // back substitution
        for i in (1..=n).rev() {
            let (mut t1, mut t2) = (0.0, 0.0);
            if i != n {
                let row = self.mi(i, 1);
                for j in i + 1..=n {
                    let (zr, zi) = (self.zr[row + j - 1] as f64, self.zi[row + j - 1] as f64);
                    t1 = t1 + zr * self.cr[j] - zi * self.cim[j];
                    t2 = t2 + zr * self.cim[j] + zi * self.cr[j];
                }
            }
            let ii = self.mi(i, i);
            let (zr32, zi32) = (self.zr[ii], self.zi[ii]);
            let t = (zr32 * zr32 + zi32 * zi32) as f64;
            let (zr, zi) = (zr32 as f64, zi32 as f64);
            let t1 = self.cr[i] - t1;
            let t2 = self.cim[i] - t2;
            self.cr[i] = (t1 * zr + t2 * zi) / t;
            self.cim[i] = (t2 * zr - t1 * zi) / t;
        }
    }

    /// Source data (L477): impedance, power and SWR at each source.
    fn source_data(&mut self) {
        self.pwr = 0.0;
        self.solutions.clear();
        for src in &self.sources {
            let (crd, ci) = (self.cr[src.pulse], self.cim[src.pulse]);
            let t = crd * crd + ci * ci;
            let t1 = (src.vr * crd + src.vi * ci) / t;
            let t2 = (src.vi * crd - src.vr * ci) / t;
            let o2 = (src.vr * crd + src.vi * ci) / 2.0;
            self.pwr += o2;
            let swr = calc_swr(t1, t2, self.swr_ro, self.swr_jxo) as f32;
            self.solutions.push(SourceSolution {
                pulse: src.pulse,
                voltage: (src.vr, src.vi),
                current: (crd, ci),
                impedance: (t1, t2),
                power: o2,
                swr: swr as f64,
            });
        }
    }
}
