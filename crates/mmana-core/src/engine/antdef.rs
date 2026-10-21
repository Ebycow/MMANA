//! The engine-side antenna definition (MMANA's `ANTDEF`) and its
//! construction from the public model, including stacking (`TMainWnd::Stack`).

use super::{NULLV, PPMAX, SMAX, to_half_width};
use crate::model::{Antenna, LoadKind, StackDirection};

#[derive(Debug, Clone, Default)]
pub(crate) struct WDef {
    pub x1: f64,
    pub y1: f64,
    pub z1: f64,
    pub x2: f64,
    pub y2: f64,
    pub z2: f64,
    pub r: f64,
    pub seg: i32,
    /// First pulse of this wire (filled by `adj_plus_no`).
    pub pno: i32,
    /// Number of pulses on this wire.
    pub pmax: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct CDef {
    pub plus: String,
    pub plus_no: i32,
    /// Stacked copy the source belongs to (0: the antenna as defined).
    pub copy: usize,
    pub ev: f64,
    pub deg: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct LDef {
    /// 0: L/C/Q, 1: R+jX, 2: S-domain.
    pub ty: i32,
    pub plus: String,
    pub plus_no: i32,
    /// Stacked copy the load belongs to (0: the antenna as defined).
    pub copy: usize,
    pub sn: i32,
    pub a: [f64; SMAX],
    pub b: [f64; SMAX],
}

#[derive(Debug, Clone)]
pub(crate) struct PDef {
    pub rr: f64,
    pub l: [f64; PPMAX],
    pub r: [f64; PPMAX],
    pub ty: i32,
}

/// One expanded (sub-divided) wire, MMANA's `WZDEF`.
#[derive(Debug, Clone, Default)]
pub(crate) struct WzDef {
    pub x1: f64,
    pub y1: f64,
    pub z1: f64,
    pub x2: f64,
    pub y2: f64,
    pub z2: f64,
    pub r: f64,
    pub seg: i32,
    /// Index of the defining wire.
    pub wno: usize,
    pub pno: i32,
    pub sno: i32,
    pub smax: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct AntDef {
    pub cfq: f64,
    pub wires: Vec<WDef>,
    /// Wires as defined; stacked copy `k` starts at wire `k * base_wires`.
    pub base_wires: usize,
    pub sources: Vec<CDef>,
    pub loads_enabled: bool,
    pub loads: Vec<LDef>,
    pub pdef: Vec<PDef>,
    pub wz: Vec<WzDef>,
    pub min_z: f64,
    /// Wavelength at the calculation frequency (set before segmentation).
    pub rmd: f64,
    pub dm1: i32,
    pub dm2: i32,
    pub ec: i32,
    pub sc: f64,
}

impl AntDef {
    /// Builds the definition the engine works on. `cfq` is the calculation
    /// frequency in MHz.
    pub fn from_model(ant: &Antenna, cfq: f64) -> Self {
        let wires = ant
            .wires
            .iter()
            .map(|w| WDef {
                x1: w.start.x,
                y1: w.start.y,
                z1: w.start.z,
                x2: w.end.x,
                y2: w.end.y,
                z2: w.end.z,
                r: w.radius,
                seg: w.segmentation.to_code(),
                pno: 0,
                pmax: 0,
            })
            .collect();
        let sources = ant
            .sources
            .iter()
            .map(|s| CDef { plus: to_half_width(&s.pulse), plus_no: 0, copy: 0, ev: s.voltage, deg: s.phase_deg })
            .collect();
        let loads = ant.loads.iter().map(|l| ldef_from_model(&l.pulse, &l.kind)).collect();
        let mut def = AntDef {
            cfq,
            wires,
            base_wires: ant.wires.len(),
            sources,
            loads_enabled: ant.loads_enabled,
            loads,
            pdef: ant.combined_wires.iter().map(pdef_from_model).collect(),
            wz: Vec::new(),
            min_z: 0.0,
            rmd: 0.0,
            dm1: ant.auto_segmentation.dm1,
            dm2: ant.auto_segmentation.dm2,
            ec: ant.auto_segmentation.ec,
            sc: ant.auto_segmentation.sc,
        };
        adj_pdef(&mut def.pdef);
        if ant.stack.is_stacked() {
            def.stack(ant);
        }
        def
    }

    /// `TMainWnd::Stack`: replicates wires, sources and loads.
    fn stack(&mut self, ant: &Antenna) {
        let st = &ant.stack;
        let base_wires = self.wires.len();
        if st.horizontal > 1 {
            let orig = std::mem::take(&mut self.wires);
            let mut yc = -(((st.horizontal - 1) as f64) * st.horizontal_spacing) / 2.0;
            for _ in 0..st.horizontal {
                for w in &orig {
                    let mut w = w.clone();
                    w.y1 += yc;
                    w.y2 += yc;
                    self.wires.push(w);
                }
                yc += st.horizontal_spacing;
            }
        }
        if st.vertical > 1 {
            let orig = std::mem::take(&mut self.wires);
            let mut zc = match st.direction {
                StackDirection::Up => 0.0,
                StackDirection::Down => -(((st.vertical - 1) as f64) * st.vertical_spacing),
                StackDirection::Centered => -(((st.vertical - 1) as f64) * st.vertical_spacing) / 2.0,
            };
            for _ in 0..st.vertical {
                for w in &orig {
                    let mut w = w.clone();
                    w.z1 += zc;
                    w.z2 += zc;
                    self.wires.push(w);
                }
                zc += st.vertical_spacing;
            }
        }
        let copies = st.vertical * st.horizontal;
        let sources = std::mem::take(&mut self.sources);
        let loads = std::mem::take(&mut self.loads);
        for i in 0..copies {
            let off = i * base_wires as i32;
            for c in &sources {
                let mut c = c.clone();
                c.plus = add_wire_no(&c.plus, off);
                c.copy = i as usize;
                c.ev /= copies as f64;
                self.sources.push(c);
            }
        }
        for i in 0..copies {
            let off = i * base_wires as i32;
            for l in &loads {
                let mut l = l.clone();
                l.plus = add_wire_no(&l.plus, off);
                l.copy = i as usize;
                self.loads.push(l);
            }
        }
    }
}

fn ldef_from_model(pulse: &str, kind: &LoadKind) -> LDef {
    let mut l =
        LDef { ty: 0, plus: to_half_width(pulse), plus_no: 0, copy: 0, sn: 0, a: [NULLV; SMAX], b: [NULLV; SMAX] };
    match kind {
        LoadKind::Lc { inductance_uh, capacitance_pf, q } => {
            l.a[0] = *inductance_uh;
            l.b[0] = *capacitance_pf;
            l.a[1] = *q;
        }
        LoadKind::Impedance { r, x } => {
            l.ty = 1;
            l.a[0] = *r;
            l.b[0] = *x;
        }
        LoadKind::SDomain { coefficients } => {
            l.ty = 2;
            for (j, [a, b]) in coefficients.iter().take(SMAX).enumerate() {
                l.a[j] = *a;
                l.b[j] = *b;
            }
            // TMainWnd::GetAntDef: order = last defined coefficient + 1
            l.sn = (0..SMAX).rev().find(|&j| l.a[j] != NULLV || l.b[j] != NULLV).map_or(0, |j| j as i32 + 1);
        }
    }
    l
}

fn pdef_from_model(c: &crate::model::CombinedWire) -> PDef {
    let mut p = PDef { rr: c.radius_key, l: [0.0; PPMAX], r: [0.007; PPMAX], ty: c.mode };
    for (j, s) in c.sections.iter().take(PPMAX).enumerate() {
        p.l[j] = s.length;
        p.r[j] = s.radius;
    }
    p
}

/// `AdjPdef`: terminates every section list with a 99999.9 m sentinel.
fn adj_pdef(pdef: &mut Vec<PDef>) {
    for i in 0..pdef.len() {
        let pp = &mut pdef[i];
        if pp.rr == 0.0 {
            pdef.truncate(i);
            return;
        }
        for mut j in 0..PPMAX {
            if pp.l[j] == 0.0 {
                if j != 0 {
                    j -= 1;
                }
                pp.l[j] = 99999.9;
                if pp.r[j] == 0.0 {
                    pp.r[j] = if j != 0 { pp.r[j - 1] } else { 0.0008 };
                }
                break;
            } else if j == PPMAX - 1 {
                pp.l[j] = 99999.9;
            }
            if pp.r[j] == 0.0 {
                pp.r[j] = if j != 0 { pp.r[j - 1] } else { 0.0008 };
            }
        }
    }
}

/// `TMainWnd::AddWireNo`: shifts the wire number inside a pulse name.
/// The original adds the wire offset to a bare pulse number too, which
/// moves it onto another pulse of the first copy; numbers are left as they
/// are and resolved per copy once the pulses are known (`adj_plus_no`).
fn add_wire_no(t: &str, off: i32) -> String {
    if off == 0 {
        return t.to_owned();
    }
    let mut chars = t.chars();
    match chars.next() {
        Some(w @ ('w' | 'W')) => {
            let rest = chars.as_str();
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            let d: i32 = rest[..digits].parse().unwrap_or(0);
            format!("{w}{}{}", d + off, &rest[digits..])
        }
        _ => t.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_wire_no_shifts_wire_numbers() {
        assert_eq!(add_wire_no("w1c", 4), "w5c");
        assert_eq!(add_wire_no("W12b3", 10), "W22b3");
        assert_eq!(add_wire_no("7", 3), "7");
        assert_eq!(add_wire_no("w1c", 0), "w1c");
    }
}
