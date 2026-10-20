//! Lumped-circuit helpers: SWR, parallel resonator impedance and conductor
//! loss (`CalcSWR`, `CalcParaZ`, `GetWireRoss` in ComLib.cpp).

use super::antdef::{AntDef, WDef};
use super::segment::comb_wire;
use super::{NULLV, PAI2};
use crate::model::WireMaterial;

/// `CalcSWR`: SWR of load `rl + j xl` against reference `ro + j xo`.
pub fn calc_swr(rl: f64, xl: f64, ro: f64, xo: f64) -> f64 {
    if rl == NULLV || xl == NULLV {
        return NULLV;
    }
    let xl = xl - xo;
    let l = (ro + rl) * (ro + rl) + xl * xl;
    let a = (rl * rl + xl * xl - ro * ro) / l;
    let b = (2.0 * xl * ro) / l;
    let l = (a * a + b * b).sqrt();
    if l >= 1.0 { 99999.9 } else { (1.0 + l) / (1.0 - l) }
}

/// `CalcParaZ`: parallel L (µH, with Q) and C (pF) at `f` MHz → (R, X).
pub fn calc_para_z(f: f64, l: f64, c: f64, q: f64) -> (f64, f64) {
    let wl = PAI2 * f * l;
    let wc = PAI2 * f * c * 1.0e-6;
    let r = if q != 0.0 { wl / q } else { 0.0 };
    let a = r / (r * r + wl * wl);
    let b = wc - wl / (r * r + wl * wl);
    let y = (a * a) + (b * b);
    if y == 0.0 { (f64::MAX, -f64::MAX) } else { (a / y, -b / y) }
}

/// `Teira`: polynomial evaluation used by the skin-effect fits.
fn teira(x: f64, p: &[f64]) -> f64 {
    let mut y = p[0];
    for (i, c) in p.iter().enumerate().skip(1) {
        y += x.powf(i as f64) * c;
    }
    y
}

/// `GetWireRoss(ap, wp)`: RF resistance of a wire (summed over the sections
/// of a combined wire).
pub(crate) fn wire_loss(ap: &AntDef, wp: &WDef, material: WireMaterial) -> f64 {
    if wp.r < 0.0 {
        if let Some(pp) = ap.pdef.iter().find(|p| p.rr == wp.r) {
            return comb_wire(ap, pp, wp).iter().map(|w| wire_loss_f(ap.cfq, w, material)).fold(0.0, |a, b| a + b);
        }
    }
    wire_loss_f(ap.cfq, wp, material)
}

/// `GetWireRoss(f, wp)`
fn wire_loss_f(f: f64, wp: &WDef, material: WireMaterial) -> f64 {
    let xx = wp.x2 - wp.x1;
    let yy = wp.y2 - wp.y1;
    let zz = wp.z2 - wp.z1;
    let l = (xx * xx + yy * yy + zz * zz).sqrt();
    let mut s = wp.r * wp.r * std::f64::consts::PI;
    let (lo, us) = match material {
        WireMaterial::Copper | WireMaterial::CopperPipe => (1.8e-8, 1.0 - 0.96e-5),
        WireMaterial::Aluminum | WireMaterial::AluminumPipe => (2.8e-8, 1.0 + 2.2e-5),
        WireMaterial::Iron | WireMaterial::IronPipe => (9.4e-8, 150.0),
        WireMaterial::Lossless => return 0.0,
    };
    let pipe = matches!(material, WireMaterial::CopperPipe | WireMaterial::AluminumPipe | WireMaterial::IronPipe);
    let mut r;
    if pipe {
        let t = wp.r * 0.9;
        s -= t * t * std::f64::consts::PI;
        r = l * lo / s;
        let x = 0.1985 * wp.r * 10.0 * (us * f * 1e6 / (lo * 1e8)).sqrt();
        if x >= 4.0 {
            r *= x;
        } else if x >= 1.0 {
            const TT: [f64; 8] = [
                -4.772531e-01,
                5.851077e+00,
                -9.276207e+00,
                7.476381e+00,
                -3.116668e+00,
                7.079749e-01,
                -8.326961e-02,
                3.965883e-03,
            ];
            r *= teira(x, &TT);
        } else if x >= 0.3 {
            const TT: [f64; 8] = [
                3.760000e-01,
                8.423095e+00,
                -4.696111e+01,
                1.402500e+02,
                -2.425694e+02,
                2.437500e+02,
                -1.319444e+02,
                2.976190e+01,
            ];
            r *= teira(x, &TT);
        }
    } else {
        r = l * lo / s;
        let x = 28.1 * wp.r * (us * f * 1e6 / (lo * 1e8)).sqrt();
        if x >= 100.0 {
            r *= x * 0.3561;
        } else if x >= 10.0 {
            const TT: [f64; 10] = [
                -2.120000e+00,
                1.004767e+00,
                -7.089785e-02,
                4.119507e-03,
                -1.430243e-04,
                3.112024e-06,
                -4.280139e-08,
                3.612087e-10,
                -1.706349e-12,
                3.452932e-15,
            ];
            r *= teira(x, &TT);
        } else if x >= 1.0 {
            const TT: [f64; 11] = [
                1.384010e+00,
                -1.388426e+00,
                2.087645e+00,
                -1.712435e+00,
                8.488221e-01,
                -2.589923e-01,
                5.010682e-02,
                -6.182203e-03,
                4.721516e-04,
                -2.036364e-05,
                3.794671e-07,
            ];
            r *= teira(x, &TT);
        }
    }
    r
}
