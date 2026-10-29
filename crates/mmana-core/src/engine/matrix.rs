//! S196: impedance matrix fill, loads, factorisation and solution.
//!
//! The matrix is stored as `f32` exactly like the original (`float *aZR`), and
//! every expression keeps the original's mix of single- and double-precision
//! arithmetic: products of two matrix elements are evaluated in `f32`, products
//! with a `double` are promoted. This is what makes the results bit-compatible.

use super::circuit::calc_swr;
use super::{Mininec, SourceSolution, sgn};

/// Subtracts (T1 + jT2) × row K from row I (L413-L421), rounding to `f32`.
#[inline]
fn update_row(row_r: &mut [f32], row_i: &mut [f32], k_r: &[f32], k_i: &[f32], (t1, t2): (f64, f64)) {
    for ((zr, zi), (&zrk, &zik)) in row_r.iter_mut().zip(row_i.iter_mut()).zip(k_r.iter().zip(k_i)) {
        let (zrk, zik) = (zrk as f64, zik as f64);
        *zr = (*zr as f64 - (zrk * t1 - zik * t2)) as f32;
        *zi = (*zi as f64 - (zrk * t2 + zik * t1)) as f32;
    }
}

/// Scalar-potential PSI(P1, N, N+1) values (S87) of two observation points,
/// by source segment start N. The fill asks for each value up to four times
/// (neighbouring columns, and neighbouring rows on one wire); S87 depends only
/// on its arguments, so the stored value is exactly what the original
/// recomputes each time.
struct PsiCache {
    slots: [PsiSlot; 2],
}

struct PsiSlot {
    /// P1, K and wire of the observation pulse.
    key: Option<(f64, i32, i32)>,
    /// By N.
    vals: Vec<Option<PsiEntry>>,
}

/// (S87 also reads the wire of the source pulse, but N is a point of one
/// wire: the pulses that ask for it, the one starting at N and the one
/// before, are on that wire.)
#[derive(Clone, Copy)]
struct PsiEntry {
    p4: usize,
    psi: (f64, f64),
}

impl PsiCache {
    fn new(break_points: usize) -> Self {
        let slot = || PsiSlot { key: None, vals: vec![None; break_points] };
        Self { slots: [slot(), slot()] }
    }

    /// (`p3` is always `p2 + 1`, see [`Mininec::s87`].)
    #[allow(clippy::too_many_arguments)]
    fn s87(&mut self, e: &Mininec, p1: f64, p2: f64, p3: f64, p4: usize, k: i32, obs: usize, src: usize) -> (f64, f64) {
        let key = Some((p1, k, e.wi[obs]));
        let slot = match self.slots.iter().position(|s| s.key == key) {
            Some(s) => &mut self.slots[s],
            None => {
                // rows go up the wire: the lower observation point is done with
                let [a, b] = &mut self.slots;
                let slot = match (a.key, b.key) {
                    (None, _) => a,
                    (_, None) => b,
                    (Some(ka), Some(kb)) => {
                        if ka.0 <= kb.0 {
                            a
                        } else {
                            b
                        }
                    }
                };
                slot.key = key;
                slot.vals.fill(None);
                slot
            }
        };
        let n = p2 as usize;
        if let Some(c) = slot.vals[n]
            && c.p4 == p4
        {
            return c.psi;
        }
        let psi = e.s87(p1, p2, p3, p4, k, obs, src);
        slot.vals[n] = Some(PsiEntry { p4, psi });
        psi
    }
}

/// Marks a matrix element that no copy is made into.
const NO_SOURCE: u32 = u32::MAX;

/// `POW`: integer power by repeated multiplication (not `powi`, whose
/// rounding differs). The loads raise to no negative power.
fn pow_n(d: f64, n: i32) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let mut dd = d;
    for _ in 1..n {
        dd *= d;
    }
    dd
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
    ///
    /// The original fills the elements one by one and, to avoid redundant
    /// work, copies some k = +1 values forward to elements of later rows
    /// (L317-L329); an element holding a non-zero copy is not computed. Which
    /// elements receive copies depends only on the geometry, so the fill runs
    /// in four steps that produce exactly the same matrix:
    /// 1. find the copy targets (sequential, cheap);
    /// 2. compute the k = +1 term of every other element (parallel by row);
    /// 3. replay the copies in the original order;
    /// 4. add the ground image (k = -1) term (parallel by row).
    fn fill(&mut self) {
        let n = self.n;
        let mut zr = vec![0.0f32; n * n];
        let mut zi = vec![0.0f32; n * n];
        let src = self.copy_sources();
        let rows: Vec<(f64, f64, f64)> =
            (0..=n).map(|i| if i == 0 { (0.0, 0.0, 0.0) } else { self.row_terms(i) }).collect();

        // 2. k = +1 term of the elements that are not copied
        self.fill_rows(&mut zr, &mut zi, |i, row_r, row_i, psi| {
            for j in 1..=n {
                if src[(i - 1) * n + j - 1] != NO_SOURCE {
                    continue;
                }
                let (dr, di) = self.element(i, j, 1, self.f8(i, j), rows[i], psi);
                Self::add_f64(row_r, j - 1, dr);
                Self::add_f64(row_i, j - 1, di);
            }
        });

        // 3. copies, in the original order (a copy can itself be copied on)
        for (ij, &s) in src.iter().enumerate() {
            if s == NO_SOURCE {
                continue;
            }
            let s = s as usize;
            zr[ij] = zr[s];
            zi[ij] = zi[s];
            if zr[ij] == 0.0 {
                // the original computes elements whose copy is zero
                let (i, j) = (ij / n + 1, ij % n + 1);
                let psi = &mut PsiCache::new(self.x.len());
                let (dr, di) = self.element(i, j, 1, self.f8(i, j), rows[i], psi);
                Self::add_f64(&mut zr, ij, dr);
                Self::add_f64(&mut zi, ij, di);
            }
        }

        // 4. ground image
        if self.g < 0 {
            self.fill_rows(&mut zr, &mut zi, |i, row_r, row_i, psi| {
                for j in 1..=n {
                    let [_, cj1, cj2] = self.ci[j];
                    if cj1 == -cj2 {
                        continue;
                    }
                    let (dr, di) = self.element(i, j, -1, 0, rows[i], psi);
                    Self::add_f64(row_r, j - 1, dr);
                    Self::add_f64(row_i, j - 1, di);
                }
            });
        }
        self.zr = zr;
        self.zi = zi;
    }

    /// Calls `body(i, row_r, row_i, psi)` for every row `i`, in parallel blocks
    /// of consecutive rows that share one `PsiCache`.
    fn fill_rows(
        &self,
        zr: &mut [f32],
        zi: &mut [f32],
        body: impl Fn(usize, &mut [f32], &mut [f32], &mut PsiCache) + Sync + Send,
    ) {
        let n = self.n;
        let block = (n / 64).clamp(1, 16);
        crate::par::rows_mut(zr, zi, n * block, |b, block_r, block_i| {
            let mut psi = PsiCache::new(self.x.len());
            for (r, (row_r, row_i)) in block_r.chunks_mut(n).zip(block_i.chunks_mut(n)).enumerate() {
                body(b * block + r + 1, row_r, row_i, &mut psi);
            }
        });
    }

    /// R(M + 1/2) - R(M - 1/2) = (T5, T6, T7) for observation pulse `i`.
    fn row_terms(&self, i: usize) -> (f64, f64, f64) {
        let [_, ci1, ci2] = self.ci[i];
        let i1 = ci1.unsigned_abs() as usize;
        let i2 = ci2.unsigned_abs() as usize;
        let f4 = sgn(ci1) as f64 * self.s[i1];
        let f5 = sgn(ci2) as f64 * self.s[i2];
        let t5 = f4 * self.ca[i1] + f5 * self.ca[i2];
        let t6 = f4 * self.cb[i1] + f5 * self.cb[i2];
        let mut t7 = f4 * self.cg[i1] + f5 * self.cg[i2];
        if ci1 == -ci2 {
            t7 = self.s[i1] * (self.cg[i1] + self.cg[i2]);
        }
        (t5, t6, t7)
    }

    /// Whether pulse `i` lies within one straight wire (part of the test that
    /// sets the redundancy flag F8).
    fn uniform_pulse(&self, i: usize) -> bool {
        let [_, c1, c2] = self.ci[i];
        let (i1, i2) = (c1.unsigned_abs() as usize, c2.unsigned_abs() as usize);
        i1 == i2 && ((self.ca[i1] + self.cb[i1]) == 0.0 || c1 == c2)
    }

    /// Redundancy flag F8 of the k = +1 pass (L235-L246): 2 on the diagonal,
    /// 1 for two uniform pulses of the same wire, otherwise 0.
    fn f8(&self, i: usize, j: usize) -> i32 {
        if !(self.uniform_pulse(i) && self.uniform_pulse(j))
            || self.ci[i][1].unsigned_abs() != self.ci[j][1].unsigned_abs()
        {
            return 0;
        }
        if i == j { 2 } else { 1 }
    }

    /// For each matrix element, the element whose k = +1 value the original
    /// copies into it last (L317-L329), or `NO_SOURCE`.
    fn copy_sources(&self) -> Vec<u32> {
        let n = self.n;
        let mut src = vec![NO_SOURCE; n * n];
        for i in 1..=n {
            if !self.uniform_pulse(i) {
                continue;
            }
            for j in i..=n {
                if self.f8(i, j) == 0 {
                    continue;
                }
                let ij = self.mi(i, j) as u32;
                if j != i {
                    src[self.mi(j, i)] = ij;
                }
                // segments on same wire same distance apart have same Z
                let p1 = j + 1;
                if p1 > n {
                    continue;
                }
                let cp = self.ci[p1];
                let cj2 = self.ci[j][2];
                if cp[1] != cp[2] {
                    continue;
                }
                if cp[2] != cj2 {
                    if cp[2] != -cj2 {
                        continue;
                    }
                    let j2 = cj2.unsigned_abs() as usize;
                    if (self.ca[j2] + self.cb[j2]) != 0.0 {
                        continue;
                    }
                }
                let p2 = i + 1;
                if p2 > n {
                    continue;
                }
                src[self.mi(p1, p2)] = ij;
            }
        }
        src
    }

    /// Contribution of image `k` to element (`i`, `j`) (body of L235-L314).
    fn element(&self, i: usize, j: usize, k: i32, f8: i32, t: (f64, f64, f64), psi: &mut PsiCache) -> (f64, f64) {
        let [_, cj1, cj2] = self.ci[j];
        let j1 = cj1.unsigned_abs() as usize;
        let j2 = cj2.unsigned_abs() as usize;
        let f4 = sgn(cj1) as f64;
        let f5 = sgn(cj2) as f64;
        let (f6, f7) = if cj1 == -cj2 { (f4, f5) } else { (1.0, 1.0) };
        self.compute_element(i, j, k, (j1, j2), (f4, f5, f6, f7), f8, t, psi)
    }

    /// L248-L314: one matrix element contribution.
    #[allow(clippy::too_many_arguments)]
    fn compute_element(
        &self,
        i: usize,
        j: usize,
        k: i32,
        (j1, j2): (usize, usize),
        (f4, f5, f6, f7): (f64, f64, f64, f64),
        f8: i32,
        (t5, t6, t7): (f64, f64, f64),
        psi: &mut PsiCache,
    ) -> (f64, f64) {
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
                (t1, t2) = psi.s87(self, p1, p2, p3, j2, k, i, j);
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
            (t1, t2) = psi.s87(self, p1, p2, p3, j2, k, i, j);
            u1 = (t1 - u5) / self.s[j2];
            u2 = (t2 - u6) / self.s[j2];
            // compute PSI(M+1/2, N-1, N)
            p1 += 1.0;
            p3 = p2;
            p2 -= 1.0;
            (t1, t2) = psi.s87(self, p1, p2, p3, j1, k, i, j);
            let u3 = t1;
            let u4 = t2;
            // compute PSI(M-1/2, N-1, N)
            if f8 >= 1 {
                t1 = u5;
                t2 = u6;
            } else {
                p1 -= 1.0;
                (t1, t2) = psi.s87(self, p1, p2, p3, j1, k, i, j);
            }
            // gradient of scalar potential contribution
            u1 += (u3 - t1) / self.s[j1];
            u2 += (u4 - t2) / self.s[j1];
        }
        // term to sum into the impedance matrix
        (kf * (d1 + u1), kf * (d2 + u2))
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
                    // (`a * s * w` as `a * (s * w)`, the same: s is ±1)
                    let (w0, w1) = (s * pow_n(f5, jj), s * pow_n(f5, jj + 1));
                    u1 = u1 + ld.la[1][j] * w0;
                    d1 = d1 + ld.la[2][j] * w0;
                    u2 = u2 + ld.la[1][j + 1] * w1;
                    d2 = d2 + ld.la[2][j + 1] * w1;
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
        factor(&mut self.zr, &mut self.zi, self.n)
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

/// Gaussian elimination with partial pivoting (L377-L426) of the N×N matrix
/// `zr + j·zi`, in place; returns the pivot rows.
///
/// The original applies step K to the whole trailing matrix before step
/// K + 1. Here the steps are taken `PANEL` at a time: the panel's columns
/// are eliminated as in the original, then the rest of each row receives
/// the panel's steps in the same order, each rounded to `f32` as before.
/// Every element goes through exactly the original operations; only the
/// loop order changes, which keeps the rows in cache and lets them be
/// updated in parallel.
fn factor(zr: &mut [f32], zi: &mut [f32], n: usize) -> Vec<usize> {
    const PANEL: usize = 32;
    let mut piv = vec![0usize; n + 1];
    // multipliers (T1, T2) of each row for the steps of the current panel
    let mut mul = vec![(0.0f64, 0.0f64); (n + 1) * PANEL];
    let mut k0 = 1;
    while k0 < n {
        let ke = (k0 + PANEL - 1).min(n - 1);
        for k in k0..=ke {
            // search for pivot
            let kk = (k - 1) * n + k - 1;
            let mut t = (zr[kk] * zr[kk] + zi[kk] * zi[kk]) as f64;
            let mut i1 = k;
            // (from K itself, which keeps I1 = K, as the original from K + 1)
            for i in k..=n {
                let ik = (i - 1) * n + k - 1;
                let t1 = (zr[ik] * zr[ik] + zi[ik] * zi[ik]) as f64;
                if t1 >= t {
                    i1 = i;
                    t = t1;
                }
            }
            // exchange rows K and I1, with their pending multipliers
            if i1 != k {
                let (a, b) = ((k - 1) * n, (i1 - 1) * n);
                for c in 0..n {
                    zr.swap(a + c, b + c);
                    zi.swap(a + c, b + c);
                }
                for p in 0..k - k0 {
                    mul.swap(k * PANEL + p, i1 * PANEL + p);
                }
            }
            piv[k] = i1;
            // subtract row K from rows K+1 to N (panel columns only)
            let (c, d) = (zr[kk], zi[kk]);
            for i in k + 1..=n {
                let ik = (i - 1) * n + k - 1;
                let (a, b) = (zr[ik], zi[ik]);
                let t1 = (a * c + b * d) as f64 / t;
                let t2 = (b * c - a * d) as f64 / t;
                zr[ik] = t1 as f32;
                zi[ik] = t2 as f32;
                mul[i * PANEL + k - k0] = (t1, t2);
                for col in k..ke {
                    let (ix, kx) = ((i - 1) * n + col, (k - 1) * n + col);
                    let (zrk, zik) = (zr[kx] as f64, zi[kx] as f64);
                    zr[ix] = (zr[ix] as f64 - (zrk * t1 - zik * t2)) as f32;
                    zi[ix] = (zi[ix] as f64 - (zrk * t2 + zik * t1)) as f32;
                }
            }
        }
        // trailing columns of the panel rows: each row needs the rows above it
        for k in k0 + 1..=ke {
            let (above_r, row_r) = zr.split_at_mut((k - 1) * n);
            let (above_i, row_i) = zi.split_at_mut((k - 1) * n);
            for p in k0..k {
                let off = (p - 1) * n;
                update_row(
                    &mut row_r[ke..n],
                    &mut row_i[ke..n],
                    &above_r[off + ke..off + n],
                    &above_i[off + ke..off + n],
                    mul[k * PANEL + p - k0],
                );
            }
        }
        // trailing columns of the rows below the panel, in parallel
        let (head_r, tail_r) = zr.split_at_mut(ke * n);
        let (head_i, tail_i) = zi.split_at_mut(ke * n);
        let mul = &mul;
        crate::par::rows_mut(tail_r, tail_i, n, |r, row_r, row_i| {
            let i = ke + 1 + r;
            for p in k0..=ke {
                let off = (p - 1) * n;
                update_row(
                    &mut row_r[ke..],
                    &mut row_i[ke..],
                    &head_r[off + ke..off + n],
                    &head_i[off + ke..off + n],
                    mul[i * PANEL + p - k0],
                );
            }
        });
        k0 = ke + 1;
    }
    piv
}

#[cfg(test)]
mod tests {
    use super::factor;

    /// The original loop order (L377-L426), one step K at a time.
    fn factor_reference(zr: &mut [f32], zi: &mut [f32], n: usize) -> Vec<usize> {
        let mut piv = vec![0usize; n + 1];
        let mi = |r: usize, c: usize| (r - 1) * n + (c - 1);
        for k in 1..n {
            let kk = mi(k, k);
            let mut t = (zr[kk] * zr[kk] + zi[kk] * zi[kk]) as f64;
            let mut i1 = k;
            for i in k + 1..=n {
                let ik = mi(i, k);
                let t1 = (zr[ik] * zr[ik] + zi[ik] * zi[ik]) as f64;
                if t1 >= t {
                    i1 = i;
                    t = t1;
                }
            }
            if i1 != k {
                for c in 1..=n {
                    zr.swap(mi(k, c), mi(i1, c));
                    zi.swap(mi(k, c), mi(i1, c));
                }
            }
            piv[k] = i1;
            for i in k + 1..=n {
                let ik = mi(i, k);
                let (a, b, c, d) = (zr[ik], zi[ik], zr[kk], zi[kk]);
                let t1 = (a * c + b * d) as f64 / t;
                let t2 = (b * c - a * d) as f64 / t;
                zr[ik] = t1 as f32;
                zi[ik] = t2 as f32;
                for c in k + 1..=n {
                    let (ix, kx) = (mi(i, c), mi(k, c));
                    let (zrk, zik) = (zr[kx] as f64, zi[kx] as f64);
                    zr[ix] = (zr[ix] as f64 - (zrk * t1 - zik * t2)) as f32;
                    zi[ix] = (zi[ix] as f64 - (zrk * t2 + zik * t1)) as f32;
                }
            }
        }
        piv
    }

    #[test]
    fn blocked_factor_matches_original_loop_order() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
        };
        // sizes around the panel width, with and without a dominant diagonal
        // (the latter pivots on almost every step)
        for n in [1, 2, 3, 31, 32, 33, 63, 64, 65, 100, 197] {
            for dominant in [false, true] {
                let mut zr: Vec<f32> = (0..n * n).map(|_| rnd()).collect();
                let mut zi: Vec<f32> = (0..n * n).map(|_| rnd()).collect();
                if dominant {
                    for i in 0..n {
                        zr[i * n + i] += 10.0;
                    }
                }
                let (mut rr, mut ri) = (zr.clone(), zi.clone());
                let piv = factor(&mut zr, &mut zi, n);
                let piv_ref = factor_reference(&mut rr, &mut ri, n);
                assert_eq!(piv, piv_ref, "pivots, n = {n}");
                let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
                assert_eq!(bits(&zr), bits(&rr), "real part, n = {n}");
                assert_eq!(bits(&zi), bits(&ri), "imaginary part, n = {n}");
            }
        }
    }
}
