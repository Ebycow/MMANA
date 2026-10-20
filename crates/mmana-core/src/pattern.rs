//! Radiation-pattern scan and figures of merit (`CalCurrent` in mininec3.cpp
//! and the pattern part of `CRes` in ComLib.cpp).
//!
//! The scan is the original's four passes: front and back vertical cuts at
//! azimuth 0, a 0.1° refinement around the vertical peak to find the take-off
//! angle, then a horizontal cut at that elevation. Values are kept as `f32`
//! like the original's result arrays.

use crate::engine::{FarFieldSample, Mininec};

/// Accumulates pattern samples exactly like `CRes`.
pub(crate) struct PatternState {
    /// Degrees per stored sample (1 or 2).
    pub ptn_f: i32,
    voff: i32,
    deg_f: i32,
    deg_v: i32,
    /// [vertical pol, horizontal pol, total][index], dBi.
    pub ptn_v: [[f32; 181]; 3],
    pub ptn_h: [[f32; 360]; 3],
    pub max_h: f32,
    pub max_v: f32,
    pub max_g: f32,
    pub max_ev: f32,
    pub max_vp: f32,
    pub max_hp: f32,
    pub elv: f32,
    /// Elevation of the horizontal cut.
    pub elv_h_ptn: f32,
    pub fb: f32,
}

impl PatternState {
    pub fn new(ptn_f: i32) -> Self {
        Self {
            ptn_f,
            voff: 0,
            deg_f: 0,
            deg_v: 0,
            ptn_v: [[0.0; 181]; 3],
            ptn_h: [[0.0; 360]; 3],
            max_h: 0.0,
            max_v: 0.0,
            max_g: 0.0,
            max_ev: 0.0,
            max_vp: 0.0,
            max_hp: 0.0,
            elv: 0.0,
            elv_h_ptn: 0.0,
            fb: 0.0,
        }
    }

    fn set_vert(&mut self, off: i32) {
        self.deg_f = 1;
        self.voff = off;
    }

    fn set_hori(&mut self) {
        self.voff = 0;
        self.deg_f = 0;
    }

    fn set_elv(&mut self, off: i32) {
        self.max_ev = self.max_v;
        self.voff = off * 10;
        self.deg_f = 2;
        let mut dv = self.deg_v as f64;
        if dv >= (90 / self.ptn_f) as f64 {
            dv = (180 / self.ptn_f) as f64 - dv;
        }
        self.elv = (dv * self.ptn_f as f64) as f32;
    }

    fn set_ptn(&mut self, s: FarFieldSample) {
        let (s1, s2, s3) = (s.vertical_db as f32, s.horizontal_db as f32, s.total_db as f32);
        let v = self.voff as usize;
        match self.deg_f {
            0 => {
                if v < 360 {
                    self.ptn_h[0][v] = s1;
                    self.ptn_h[1][v] = s2;
                    self.ptn_h[2][v] = s3;
                }
            }
            1 => {
                if v < 181 {
                    self.ptn_v[0][v] = s1;
                    self.ptn_v[1][v] = s2;
                    self.ptn_v[2][v] = s3;
                }
            }
            _ => {
                if self.max_ev < s3 {
                    let d = self.deg_v as usize;
                    self.ptn_v[0][d] = s1;
                    self.ptn_v[1][d] = s2;
                    self.ptn_v[2][d] = s3;
                    self.max_ev = s3;
                    self.elv = (self.voff as f64 / 10.0) as f32;
                }
            }
        }
        self.voff += 1;
    }

    fn get_vmax(&mut self) -> i32 {
        self.max_v = -999999.0;
        self.deg_v = 0;
        let n = 180 / self.ptn_f;
        for i in 0..=n {
            let p = self.ptn_v[2][i as usize];
            if self.max_v < p || (i > n / 2 && self.max_v == p) {
                self.max_v = p;
                self.deg_v = i;
            }
        }
        self.deg_v * self.ptn_f
    }

    fn get_elv(&mut self) -> f64 {
        if self.elv >= 90.0 {
            self.elv = 180.0 - self.elv;
        }
        self.elv as f64
    }

    /// `GetAdjElv`: elevation used for the horizontal cut.
    fn get_adj_elv(&mut self, fix_to_zero: bool) -> f64 {
        let mut vv = self.get_elv();
        if fix_to_zero {
            vv = 0.0;
        } else if vv >= 87.0 {
            vv = 45.0;
        }
        self.elv_h_ptn = vv as f32;
        vv
    }

    fn horizontal_maxima(&mut self) {
        self.max_vp = self.ptn_h[0][0];
        self.max_hp = self.ptn_h[1][0];
        self.max_h = self.ptn_h[2][0];
        let n = (360 / self.ptn_f) as usize;
        for i in 1..n {
            if self.max_vp < self.ptn_h[0][i] {
                self.max_vp = self.ptn_h[0][i];
            }
            if self.max_hp < self.ptn_h[1][i] {
                self.max_hp = self.ptn_h[1][i];
            }
            if self.max_h < self.ptn_h[2][i] {
                self.max_h = self.ptn_h[2][i];
            }
        }
        self.max_g = self.max_v;
        if self.max_g < self.max_ev {
            self.max_g = self.max_ev;
        }
        if self.max_g < self.max_h {
            self.max_g = self.max_h;
        }
    }

    /// `CalcFB`: front (azimuth 0) minus the strongest back-lobe value within
    /// `fbr` (horizontal | vertical << 16 degrees).
    fn calc_fb(&mut self, fbr: i32) {
        let pf = self.ptn_f;
        let mut bd = self.ptn_h[2][(180 / pf) as usize];
        if fbr != 0 {
            let fbrh = fbr & 0x0000ffff;
            let fbrv = (fbr >> 16) & 0x0000ffff;
            let mut n = (180 - (fbrh / 2)) / pf;
            let mut i = 0;
            while i <= fbrh {
                if let Some(&p) = self.ptn_h[2].get(n as usize)
                    && bd < p
                {
                    bd = p;
                }
                i += pf;
                n += 1;
            }
            let mut i = 0;
            let mut n = 0;
            while i <= fbrv {
                if let Some(&p) = self.ptn_v[2].get(n as usize)
                    && bd < p
                {
                    bd = p;
                }
                i += pf;
                n += 1;
            }
        }
        self.fb = self.ptn_h[2][0] - bd;
    }

    /// `CalcPtn`
    fn calc_ptn(&mut self, fbr: i32) {
        self.horizontal_maxima();
        self.calc_fb(fbr);
        if self.elv >= 90.0 {
            self.elv = 180.0 - self.elv;
        }
    }
}

/// `CalCurrent` (1°) / `CalCurrentDR` (2°) after the currents are solved.
pub(crate) fn scan(e: &Mininec, ptn_f: i32, fix_elevation_to_zero: bool, fbr: i32) -> PatternState {
    let mut r = PatternState::new(ptn_f);
    let pf = ptn_f as f64;
    r.set_vert(if ptn_f == 1 { 90 } else { 45 });
    e.far_field(0.0, pf, 91, 0.0, 0.0, 0, |s| r.set_ptn(s));
    r.set_vert(0);
    e.far_field(270.0, pf, 90, 0.0, 0.0, 0, |s| r.set_ptn(s));

    let mut v = r.get_vmax();
    v = if v >= 90 { v - 90 } else { 90 - v };
    // refine the take-off angle in 0.1° steps
    v += 1;
    if v >= 90 {
        v = 90;
    }
    r.set_elv(90 - v);
    e.far_field(v as f64, -0.1, 21, 0.0, 0.0, 0, |s| r.set_ptn(s));

    let mut vv = r.get_adj_elv(fix_elevation_to_zero);
    vv = if vv >= 90.0 { vv - 90.0 } else { 90.0 - vv };
    r.set_hori();
    e.far_field(vv, 0.0, 0, 0.0, pf, 360, |s| r.set_ptn(s));
    r.calc_ptn(fbr);
    r
}

/// `CalcNewPtn`: recomputes the horizontal cut at another elevation and
/// updates gain and F/B (`ReCalcGain`).
pub(crate) fn rescan_horizontal(e: &Mininec, r: &mut PatternState, elevation_deg: f64, fbr: i32) {
    r.elv_h_ptn = elevation_deg as f32;
    let vv = r.elv_h_ptn as f64;
    let vv = if vv >= 90.0 { vv - 90.0 } else { 90.0 - vv };
    r.set_hori();
    let pf = r.ptn_f as f64;
    e.far_field(vv, 0.0, 0, 0.0, pf, 360, |s| r.set_ptn(s));
    r.horizontal_maxima();
    r.calc_fb(fbr);
}
