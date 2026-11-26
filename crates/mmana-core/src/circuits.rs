//! The calculators of MMANA's 共振条件 dialog (`TOptDlgBox` in `OptDlg.cpp`):
//! LC resonance, the air-core coil, the L-network match, a stub as a
//! reactance, the transmission line, the two-section (Q) match and the stub
//! match, and the hairpin reactance of the environment page (`AutojXClick`).
//!
//! The arithmetic follows the original operation for operation, a real
//! operand of a complex product widened to `(R, 0)` as `CLX` does, so the
//! results are the original's to the bit (`tests/circuits.rs` compares them
//! with the oracle). Inputs and results are in the units of the dialog's
//! fields (MHz, Ω, µH, pF, cm, mm, wavelengths).

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::calc::Complex;
use crate::engine::circuit::calc_swr;
use crate::error::CalcError;

const PAI: f64 = std::f64::consts::PI;

/// `NULLV`: a value the dialog leaves blank.
const NULLV: f64 = 1.0e-99;

/// The reactance `CalcCore` gives an open circuit.
const OPEN: f64 = -9.999999e37;

/// `CLX`, with the operators the dialog uses. A real operand is widened to
/// `(R, 0)` and multiplied or divided as a complex number, as the original does.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Clx {
    r: f64,
    j: f64,
}

impl Clx {
    const fn new(r: f64, j: f64) -> Self {
        Self { r, j }
    }
    fn real(r: f64) -> Self {
        Self::new(r, 0.0)
    }
    /// `CLX::Mul`
    fn mul(self, o: Clx) -> Self {
        Self::new(self.r * o.r - self.j * o.j, self.r * o.j + self.j * o.r)
    }
    /// `CLX::Div`
    fn div(self, o: Clx) -> Self {
        let r = self.r * o.r + self.j * o.j;
        let j = self.j * o.r - self.r * o.j;
        let d = o.r * o.r + o.j * o.j;
        Self::new(r / d, j / d)
    }
    fn add(self, o: Clx) -> Self {
        Self::new(self.r + o.r, self.j + o.j)
    }
    fn sub(self, o: Clx) -> Self {
        Self::new(self.r - o.r, self.j - o.j)
    }
    fn scale(self, r: f64) -> Self {
        self.mul(Self::real(r))
    }
}

impl From<Clx> for Complex {
    fn from(c: Clx) -> Self {
        Complex { re: c.r, im: c.j }
    }
}

impl From<Complex> for Clx {
    fn from(c: Complex) -> Self {
        Clx::new(c.re, c.im)
    }
}

// ------------------------------------------------------------- inputs ---

fn out_of_range(field: &str, rule: &'static str, value: f64) -> CalcError {
    CalcError::OutOfRange { field: field.to_string(), rule, value }
}

fn finite(field: &str, value: f64) -> Result<f64, CalcError> {
    if value.is_finite() { Ok(value) } else { Err(out_of_range(field, "a finite number", value)) }
}

fn positive(field: &str, value: f64) -> Result<f64, CalcError> {
    if value.is_finite() && value > 0.0 { Ok(value) } else { Err(out_of_range(field, "a positive number", value)) }
}

fn non_negative(field: &str, value: f64) -> Result<f64, CalcError> {
    if value.is_finite() && value >= 0.0 { Ok(value) } else { Err(out_of_range(field, "0 or more", value)) }
}

fn impedance(field: &str, z: Complex) -> Result<Clx, CalcError> {
    non_negative(&format!("{field} resistance"), z.re)?;
    finite(&format!("{field} reactance"), z.im)?;
    Ok(z.into())
}

/// A finite value, or None (the dialog leaves the field blank).
fn shown(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

fn shown_complex(c: Clx) -> Option<Complex> {
    (c.r.is_finite() && c.j.is_finite()).then(|| c.into())
}

// ------------------------------------------------------ LC resonance ---

/// What the LC resonance is calculated from (the field last edited).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "from", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TankInput {
    /// L and C of reactance X at the frequency.
    Reactance { frequency_mhz: f64, reactance_ohm: f64 },
    /// The reactance of L, and the C that resonates with it.
    Inductance { frequency_mhz: f64, inductance_uh: f64 },
    /// The reactance of C, and the L that resonates with it.
    Capacitance { frequency_mhz: f64, capacitance_pf: f64 },
    /// The frequency L and C resonate at.
    Resonance { inductance_uh: f64, capacitance_pf: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Wavelengths {
    pub full_m: f64,
    pub half_m: f64,
    pub quarter_m: f64,
    pub five_eighths_m: f64,
}

/// L and C resonating at a frequency, with the reactance of each there.
/// The values not determined (a reactance of 0) are None.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Tank {
    pub frequency_mhz: Option<f64>,
    pub reactance_ohm: Option<f64>,
    pub inductance_uh: Option<f64>,
    pub capacitance_pf: Option<f64>,
    /// λ = 299.8 / f, and its fractions.
    pub wavelength: Option<Wavelengths>,
}

/// `CalcTank`, with the conversions of the dialog's fields.
pub fn tank(input: &TankInput) -> Result<Tank, CalcError> {
    let (mut freq, mut x, mut l, mut c) = (NULLV, NULLV, NULLV, NULLV);
    let sw = match *input {
        TankInput::Reactance { frequency_mhz, reactance_ohm } => {
            freq = positive("frequency", frequency_mhz)? * 1e6;
            x = finite("reactance", reactance_ohm)?.abs();
            0
        }
        TankInput::Inductance { frequency_mhz, inductance_uh } => {
            freq = positive("frequency", frequency_mhz)? * 1e6;
            l = positive("inductance", inductance_uh)? * 1e-6;
            1
        }
        TankInput::Capacitance { frequency_mhz, capacitance_pf } => {
            freq = positive("frequency", frequency_mhz)? * 1e6;
            c = positive("capacitance", capacitance_pf)? * 1e-12;
            2
        }
        TankInput::Resonance { inductance_uh, capacitance_pf } => {
            l = positive("inductance", inductance_uh)? * 1e-6;
            c = positive("capacitance", capacitance_pf)? * 1e-12;
            3
        }
    };
    let mut w = 2.0 * PAI * freq;
    match sw {
        0 => {
            l = NULLV;
            c = NULLV;
            if w * x != 0.0 {
                l = x / w;
                c = 1.0 / (x * w);
            }
        }
        1 => {
            x = NULLV;
            c = NULLV;
            // the original tests w * X here, X being NULLV
            if w * x != 0.0 {
                x = w * l;
                c = 1.0 / (x * w);
            }
        }
        2 => {
            x = NULLV;
            l = NULLV;
            if w * c != 0.0 {
                x = 1.0 / (c * w);
                l = x / w;
            }
        }
        _ => {
            freq = NULLV;
            x = NULLV;
            if l * c != 0.0 {
                freq = 1.0 / (2.0 * PAI * (c * l).sqrt());
                w = 2.0 * PAI * freq;
                x = w * l;
            }
        }
    }
    // SetValueItem
    let blank = |v: f64, shown_as: f64| if v == NULLV { None } else { shown(shown_as) };
    let frequency_mhz = blank(freq, freq / 1e6);
    let wavelength = frequency_mhz.map(|_| {
        let rmd = 299.8e6 / freq;
        Wavelengths { full_m: rmd, half_m: rmd / 2.0, quarter_m: rmd / 4.0, five_eighths_m: rmd * 5.0 / 8.0 }
    });
    Ok(Tank {
        frequency_mhz,
        reactance_ohm: blank(x, x),
        inductance_uh: blank(l, l * 1e6),
        capacitance_pf: blank(c, c * 1e12),
        wavelength: wavelength.filter(|w| w.full_m.is_finite()),
    })
}

// ------------------------------------------------------------- coil ---

/// Wheeler's formula, which `CalcCoil` uses, is within 1 % for coils longer
/// than this fraction of their diameter.
pub const COIL_MIN_LENGTH_RATIO: f64 = 0.4;

/// The quantity of a single-layer air-core coil calculated from the others.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "solveFor", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CoilUnknown {
    Inductance { turns: f64, diameter_cm: f64 },
    Turns { inductance_uh: f64, diameter_cm: f64 },
    Diameter { inductance_uh: f64, turns: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CoilInput {
    #[serde(flatten)]
    pub unknown: CoilUnknown,
    pub wire_diameter_mm: f64,
    /// The gap between turns (0 for close-wound).
    pub spacing_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Coil {
    pub inductance_uh: f64,
    pub turns: f64,
    pub diameter_cm: f64,
    /// (wire diameter + spacing) × turns.
    pub length_cm: f64,
    /// The coil is shorter than [`COIL_MIN_LENGTH_RATIO`] × its diameter,
    /// where the formula is off by more than 1 %.
    pub short: bool,
}

/// `CalcCoil`: Wheeler's formula with the diameter in cm,
/// L (µH) = D² N² / (45 D + 102 × length).
pub fn coil(input: &CoilInput) -> Result<Coil, CalcError> {
    let w = positive("wire diameter", input.wire_diameter_mm)? * 0.1;
    let s = non_negative("spacing", input.spacing_mm)? * 0.1;
    let (coil_l, n, d) = match input.unknown {
        CoilUnknown::Inductance { turns, diameter_cm } => {
            let n = positive("turns", turns)?;
            let d = positive("coil diameter", diameter_cm)?;
            let b = 102.0 * n * (w + s) / d;
            let coil_l = (d * n * n) / (b + 45.0);
            (coil_l * 1e-6, n, d)
        }
        CoilUnknown::Turns { inductance_uh, diameter_cm } => {
            let coil_l = positive("inductance", inductance_uh)? * 1e-6;
            let d = positive("coil diameter", diameter_cm)?;
            let ll = coil_l * 1e6;
            let b = 102.0 * ll * (w + s) / d;
            (coil_l, (b + (b * b + 180.0 * d * ll).sqrt()) / (2.0 * d), d)
        }
        CoilUnknown::Diameter { inductance_uh, turns } => {
            let coil_l = positive("inductance", inductance_uh)? * 1e-6;
            let n = positive("turns", turns)?;
            let ll = coil_l * 1e6;
            let b = 408.0 * n * n * ll * n * (w + s);
            (coil_l, n, ((45.0 * ll) + (2025.0 * ll * ll + b).sqrt()) / (2.0 * n * n))
        }
    };
    let length_cm = (w + s) * n;
    let coil = Coil {
        inductance_uh: coil_l * 1e6,
        turns: n,
        diameter_cm: d,
        length_cm,
        short: length_cm < COIL_MIN_LENGTH_RATIO * d,
    };
    if [coil.inductance_uh, n, d, length_cm].iter().all(|v| v.is_finite()) {
        Ok(coil)
    } else {
        Err(CalcError::NotRepresentable { what: "coil dimensions" })
    }
}

// ---------------------------------------------------------- LC match ---

/// Which of the two L networks: the one with a shunt C (series L when the
/// antenna's R is below the line's) or with a series C.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum LcCircuit {
    #[default]
    ShuntC,
    SeriesC,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub enum ElementKind {
    L,
    C,
}

/// A coil (µH) or a capacitor (pF) and its reactance at the frequency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Element {
    pub kind: ElementKind,
    /// µH for a coil, pF for a capacitor.
    pub value: f64,
    pub reactance_ohm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LcMatchInput {
    pub frequency_mhz: f64,
    /// The feed line's impedance.
    pub line_ohm: f64,
    /// The antenna's impedance.
    pub antenna: Complex,
    #[serde(default)]
    pub circuit: LcCircuit,
}

/// Where the shunt element sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ShuntSide {
    /// Across the line: the antenna's R is at most the line's.
    Line,
    /// Across the antenna: its R is above the line's.
    Antenna,
}

/// The L network. An element that is not needed is None (both, when the
/// antenna already matches).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LcMatch {
    pub shunt_side: ShuntSide,
    pub shunt: Option<Element>,
    pub series: Option<Element>,
}

/// `CalcMatch`, and the elements `PBoxMPaint` draws.
pub fn lc_match(input: &LcMatchInput) -> Result<LcMatch, CalcError> {
    let mf = positive("frequency", input.frequency_mhz)? * 1e6;
    let mz = positive("line impedance", input.line_ohm)?;
    let mr = positive("antenna resistance", input.antenna.re)?;
    let mx = finite("antenna reactance", input.antenna.im)?;
    let sel = input.circuit == LcCircuit::SeriesC;
    let w = 2.0 * PAI * mf;
    let none = |side| Ok(LcMatch { shunt_side: side, shunt: None, series: None });
    let (mt, mx1, mx2);
    if mx == 0.0 && mr == mz {
        return none(ShuntSide::Line);
    } else if mr <= mz {
        mt = ShuntSide::Line;
        let root = (mr * mz - mr * mr).sqrt();
        mx2 = if sel { -mx - root } else { -mx + root };
        let s = mx + mx2;
        mx1 = if s != 0.0 { -(mr * mr + s * s) / s } else { -f64::MAX };
    } else {
        mt = ShuntSide::Antenna;
        let s = 4.0 * mz * mz * mx * mx - 4.0 * (mz - mr) * mz * (mr * mr + mx * mx);
        if s < 0.0 {
            return none(mt);
        }
        let s = s.sqrt();
        let b = -2.0 * mz * mx;
        let v = 2.0 * (mz - mr);
        mx1 = if v != 0.0 { if sel { (b - s) / v } else { (b + s) / v } } else { 0.0 };
        mx2 = -(mx1 * mr * mr + (mx + mx1) * mx * mx1) / (mr * mr + (mx + mx1) * (mx + mx1));
    }
    let md1 = if mx1 >= 0.0 {
        (ElementKind::L, mx1 * 1e6 / w)
    } else if mx1 == -f64::MAX {
        (ElementKind::C, 0.0)
    } else {
        (ElementKind::C, -1e12 / (w * mx1))
    };
    let md2 = if mx2 >= 0.0 { (ElementKind::L, mx2 * 1e6 / w) } else { (ElementKind::C, -1e12 / (w * mx2)) };
    let element = |(kind, value): (ElementKind, f64), x: f64| {
        (value != 0.0 && value.is_finite() && x.is_finite()).then_some(Element { kind, value, reactance_ohm: x })
    };
    Ok(LcMatch { shunt_side: mt, shunt: element(md1, mx1), series: element(md2, mx2) })
}

// -------------------------------------------------------------- stub ---

/// The far end of a stub.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum StubEnd {
    #[default]
    Open,
    Short,
}

/// A transmission line: its impedance and velocity factor.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Cable {
    pub impedance_ohm: f64,
    pub velocity_factor: f64,
}

/// What the stub is calculated from (the field last edited).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "from", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StubGiven {
    Reactance { reactance_ohm: f64 },
    Inductance { inductance_uh: f64 },
    Capacitance { capacitance_pf: f64 },
    Length { length_cm: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StubInput {
    pub frequency_mhz: f64,
    pub cable: Cable,
    #[serde(default)]
    pub end: StubEnd,
    pub given: StubGiven,
}

/// A stub of a cable cut to a reactance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Stub {
    /// None for an open circuit (an open stub of length 0, or a capacitance of 0).
    pub reactance_ohm: Option<f64>,
    /// The coil (µH) or capacitor (pF) of the same reactance.
    pub element: Element,
    /// The shortest stub of the reactance, under half a wavelength in the cable.
    pub length_cm: f64,
    /// Where the stub is a quarter wavelength (MHz); None for a length of 0.
    pub quarter_wave_mhz: Option<f64>,
}

/// `CalcCore`, from the field `given` (`CEditXChange`, `CEditLCChange`, `CEditLChange`).
pub fn stub(input: &StubInput) -> Result<Stub, CalcError> {
    let core_freq = positive("frequency", input.frequency_mhz)? * 1e6;
    let core_z = positive("cable impedance", input.cable.impedance_ohm)?;
    let core_p = positive("velocity factor", input.cable.velocity_factor)?;
    let short = input.end == StubEnd::Short;
    let rmd = 299.8e6 * core_p / core_freq;
    let lc_from_x =
        |x: f64| if x >= 0.0 { x * 1e6 / (2.0 * PAI * core_freq) } else { -1e12 / (2.0 * PAI * core_freq * x) };
    // CalcCore(0) from L or C, CalcCore(1) from X, CalcCore(3) from the length
    let (core_x, core_lc, mut core_l, from_x) = match input.given {
        StubGiven::Inductance { inductance_uh } => {
            let core_lc = non_negative("inductance", inductance_uh)?;
            (2.0 * PAI * core_freq * core_lc * 1e-6, core_lc, 0.0, true)
        }
        StubGiven::Capacitance { capacitance_pf } => {
            let core_lc = non_negative("capacitance", capacitance_pf)?;
            let x = if core_lc != 0.0 { -1.0 / (2.0 * PAI * core_freq * core_lc * 1e-12) } else { OPEN };
            (x, core_lc, 0.0, true)
        }
        StubGiven::Reactance { reactance_ohm } => {
            let x = finite("reactance", reactance_ohm)?;
            (x, lc_from_x(x), 0.0, true)
        }
        StubGiven::Length { length_cm } => {
            let core_l = non_negative("length", length_cm)?;
            let bl = 2.0 * PAI * core_l / (rmd * 100.0);
            let x = if short {
                core_z * bl.tan()
            } else if core_l != 0.0 {
                -core_z * 1.0 / bl.tan()
            } else {
                OPEN
            };
            (x, lc_from_x(x), core_l, false)
        }
    };
    if from_x {
        let deg = if short {
            (core_x / core_z).atan() * 180.0 / PAI
        } else if core_x == 0.0 {
            90.0
        } else {
            (-core_z / core_x).atan() * 180.0 / PAI
        };
        core_l = deg * rmd * 100.0 / 360.0;
        if core_l < 0.0 {
            core_l += rmd * 100.0 / 2.0;
        }
    }
    let quarter_wave_mhz = (core_l >= 1e-5).then(|| 299.8 * core_p / (4.0 * (core_l * 0.01)));
    let kind = if core_x >= 0.0 { ElementKind::L } else { ElementKind::C };
    let stub = Stub {
        reactance_ohm: (core_x != OPEN).then_some(core_x),
        element: Element { kind, value: core_lc, reactance_ohm: core_x },
        length_cm: core_l,
        quarter_wave_mhz,
    };
    let values = [core_x, core_lc, core_l, quarter_wave_mhz.unwrap_or(0.0)];
    if values.iter().all(|v| v.is_finite()) {
        Ok(stub)
    } else {
        Err(CalcError::NotRepresentable { what: "stub values" })
    }
}

// ------------------------------------------------- transmission line ---

/// `CalcZi` (lossless): the impedance at the input of `zo` of `len`
/// wavelengths ending in `zl`.
fn line_zi(zo: f64, len: f64, zl: Clx) -> Clx {
    let len = if len >= 1.0 { len % 1.0 } else { len };
    let len = len * (2.0 * PAI);
    let (cosbl, sinbl) = (len.cos(), len.sin());
    let jo = Clx::new(0.0, 1.0).scale(zo).scale(sinbl);
    let jl = Clx::new(0.0, 1.0).mul(zl).scale(sinbl);
    Clx::real(zo).mul(zl.scale(cosbl).add(jo)).div(Clx::real(zo * cosbl).add(jl))
}

/// `CalcZi` with loss: `a` is the loss of the whole line (dB).
fn line_zi_loss(zo: f64, len: f64, zl: Clx, a: f64) -> Clx {
    let a = a / 8.685889638;
    let len = if len >= 1.0 { len % 1.0 } else { len };
    let len = len * (2.0 * PAI);
    let cos_hg = Clx::new(a.cosh() * len.cos(), a.sinh() * len.sin());
    let sin_hg = Clx::new(len.cos() * a.sinh(), len.sin() * a.cosh());
    let zo_c = Clx::real(zo);
    zo_c.mul(zl.mul(cos_hg).add(zo_c.mul(sin_hg))).div(zo_c.mul(cos_hg).add(zl.mul(sin_hg)))
}

/// `CalcZL` with loss: the load that gives `zi` at the input.
fn line_zl_loss(zo: f64, len: f64, zi: Clx, a: f64) -> Clx {
    let a = a / 8.685889638;
    let len = if len >= 1.0 { len % 1.0 } else { len };
    let len = len * (2.0 * PAI);
    let cos_hg = Clx::new(a.cosh() * len.cos(), a.sinh() * len.sin());
    let sin_hg = Clx::new(len.cos() * a.sinh(), len.sin() * a.cosh());
    let zo_c = Clx::real(zo);
    zo_c.mul(zi.mul(cos_hg).sub(zo_c.mul(sin_hg))).div(zo_c.mul(cos_hg).sub(zi.mul(sin_hg)))
}

/// Which end of the line is known.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "from", rename_all = "camelCase")]
pub enum LineEnd {
    /// The load: calculate the impedance at the input.
    Load { impedance: Complex },
    /// The input: calculate the load (ZL を計算).
    Input { impedance: Complex },
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LineInput {
    pub impedance_ohm: f64,
    pub length_wl: f64,
    /// The loss of the whole line.
    #[serde(default)]
    pub loss_db: f64,
    pub known: LineEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// None when not representable (an input impedance no load gives).
    pub input: Option<Complex>,
    pub load: Option<Complex>,
    /// Against the line's impedance.
    pub input_swr: Option<f64>,
    pub load_swr: Option<f64>,
}

/// The impedance at one end of a line from the other (分布定数線路).
pub fn line(input: &LineInput) -> Result<Line, CalcError> {
    let zo = positive("line impedance", input.impedance_ohm)?;
    let len = non_negative("line length", input.length_wl)?;
    let a = non_negative("line loss", input.loss_db)?;
    let (zi, zl) = match input.known {
        LineEnd::Load { impedance: z } => {
            let zl = impedance("load", z)?;
            (line_zi_loss(zo, len, zl, a), zl)
        }
        LineEnd::Input { impedance: z } => {
            let zi = impedance("input", z)?;
            (zi, line_zl_loss(zo, len, zi, a))
        }
    };
    let (input, load) = (shown_complex(zi), shown_complex(zl));
    Ok(Line {
        input,
        load,
        input_swr: input.and_then(|z| shown(calc_swr(z.re, z.im, zo, 0.0))),
        load_swr: load.and_then(|z| shown(calc_swr(z.re, z.im, zo, 0.0))),
    })
}

// ------------------------------------------------- two-section match ---

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LineSection {
    pub impedance_ohm: f64,
    pub length_wl: f64,
}

/// Two lines in series between a load and a feed line (Q マッチ):
/// `load_section` at the load, `feed_section` towards the feed line.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TwoSectionInput {
    pub load: Complex,
    /// The impedance the input is matched to.
    pub reference_ohm: f64,
    pub feed_section: LineSection,
    pub load_section: LineSection,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TwoSection {
    pub feed_section: LineSection,
    pub load_section: LineSection,
    /// Between the two sections.
    pub junction: Option<Complex>,
    pub input: Option<Complex>,
    /// At the input, against the reference.
    pub swr: Option<f64>,
}

struct Qm {
    rl2: Clx,
    ri2: f64,
    zo1: f64,
    zo2: f64,
}

impl Qm {
    /// `CalcQMSWR`, the part for the two sections.
    fn evaluate(&self, l1: f64, l2: f64) -> TwoSection {
        let j = line_zi(self.zo2, l2, self.rl2);
        let i = line_zi(self.zo1, l1, j);
        TwoSection {
            feed_section: LineSection { impedance_ohm: self.zo1, length_wl: l1 },
            load_section: LineSection { impedance_ohm: self.zo2, length_wl: l2 },
            junction: shown_complex(j),
            input: shown_complex(i),
            swr: shown(calc_swr(i.r, i.j, self.ri2, 0.0)),
        }
    }

    fn swr(&self, l1: f64, l2: f64) -> f64 {
        let j = line_zi(self.zo2, l2, self.rl2);
        let i = line_zi(self.zo1, l1, j);
        calc_swr(i.r, i.j, self.ri2, 0.0)
    }

    /// `CalcQML`: equal lengths on a coarse then a fine grid, then each
    /// length in turn by 0.0001 λ while the SWR falls.
    fn tune(&self) -> (f64, f64) {
        let mut ms = f64::MAX;
        let mut as_ = f64::MAX;
        let mut ml = 0.0;
        let mut l = 0.0;
        while l <= 0.5 {
            let s = self.swr(l, l);
            if s < ms {
                ms = s;
                ml = l;
            } else if as_ < s && ms < 1.5 {
                break;
            }
            as_ = s;
            l += 0.01;
        }
        as_ = f64::MAX;
        let el = ml + 0.01;
        l = ml - 0.01;
        while l < el {
            let s = self.swr(l, l);
            if s < ms {
                ms = s;
                ml = l;
            } else if as_ < s && ms < 1.05 {
                break;
            }
            as_ = s;
            l += 0.00001;
        }
        if ml < 0.0 {
            ml += 0.5;
        }
        let (mut l1, mut l2) = (ml, ml);
        let mut i = 0;
        while ms >= 1.001 && i < 30 {
            let j = line_zi(self.zo2, l2, self.rl2);
            let at = |l: f64| {
                let z = line_zi(self.zo1, l, j);
                calc_swr(z.r, z.j, self.ri2, 0.0)
            };
            ml = l1;
            ms = descend(l1, &mut ml, ms, at);
            if ml < 0.0 {
                ml += 0.5;
            }
            l1 = ml;
            ml = l2;
            ms = descend(l2, &mut ml, ms, |l| self.swr(l1, l));
            if ml < 0.0 {
                ml += 0.5;
            }
            l2 = ml;
            i += 1;
        }
        (l1, l2)
    }
}

/// The two inner searches of `CalcQML`: up from `from` by 0.0001 while the
/// SWR falls, then down from it.
fn descend(from: f64, ml: &mut f64, mut ms: f64, swr: impl Fn(f64) -> f64) -> f64 {
    let mut l = from + 0.0001;
    while l <= 0.5 {
        let s = swr(l);
        if s < ms {
            ms = s;
            *ml = l;
        } else {
            break;
        }
        l += 0.0001;
    }
    l = from - 0.0001;
    while l >= 0.0 {
        let s = swr(l);
        if s < ms {
            ms = s;
            *ml = l;
        } else {
            break;
        }
        l -= 0.0001;
    }
    ms
}

fn qm(load: Complex, reference_ohm: f64, feed_ohm: f64, load_ohm: f64) -> Result<Qm, CalcError> {
    Ok(Qm {
        rl2: impedance("load", load)?,
        ri2: positive("reference impedance", reference_ohm)?,
        zo1: positive("feed-side line impedance", feed_ohm)?,
        zo2: positive("load-side line impedance", load_ohm)?,
    })
}

/// The input of two line sections (lossless).
pub fn two_section(input: &TwoSectionInput) -> Result<TwoSection, CalcError> {
    let q = qm(input.load, input.reference_ohm, input.feed_section.impedance_ohm, input.load_section.impedance_ohm)?;
    let l1 = non_negative("feed-side line length", input.feed_section.length_wl)?;
    let l2 = non_negative("load-side line length", input.load_section.length_wl)?;
    Ok(q.evaluate(l1, l2))
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TwoSectionTuneInput {
    pub load: Complex,
    pub reference_ohm: f64,
    pub feed_impedance_ohm: f64,
    pub load_impedance_ohm: f64,
}

/// The lengths of two line sections that match the load best (TUNE).
pub fn tune_two_section(input: &TwoSectionTuneInput) -> Result<TwoSection, CalcError> {
    let q = qm(input.load, input.reference_ohm, input.feed_impedance_ohm, input.load_impedance_ohm)?;
    let (l1, l2) = q.tune();
    Ok(q.evaluate(l1, l2))
}

// -------------------------------------------------------- stub match ---

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StubMatchInput {
    pub frequency_mhz: f64,
    pub load: Complex,
    /// The impedance the input is matched to.
    pub reference_ohm: f64,
    /// The line and the stub, of the same cable.
    pub cable: Cable,
}

/// A stub across the line, cut to cancel the susceptance there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct MatchingStub {
    pub reactance_ohm: f64,
    pub element: Element,
    pub short_wl: f64,
    pub short_cm: f64,
    pub open_wl: f64,
    pub open_cm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StubMatchSolution {
    /// The distance from the load to the stub.
    pub position_wl: f64,
    pub position_cm: f64,
    /// The line's impedance there, before the stub.
    pub impedance: Complex,
    /// None when the line is already resistive there (no stub needed).
    pub stub: Option<MatchingStub>,
    /// With the stub.
    pub input: Complex,
    /// Against the reference.
    pub swr: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StubMatch {
    /// The positions within half a wavelength of the load where the SWR is
    /// lowest and under 1.5, nearest first.
    pub solutions: Vec<StubMatchSolution>,
}

/// `CalcSMStub` at `l` wavelengths from the load.
fn sm_stub(zo: f64, zl: Clx, zi_ref: f64, l: f64) -> (Clx, f64, Clx, f64, f64, f64) {
    let zi = line_zi(zo, l, zl);
    let (x, z);
    if zi.j != 0.0 {
        x = -(zi.r * zi.r + zi.j * zi.j) / zi.j;
        z = Clx::real(1.0).div(Clx::real(1.0).div(zi).add(Clx::real(1.0).div(Clx::new(0.0, x))));
    } else {
        x = f64::MAX;
        z = zi;
    }
    let mut ls = (x / zo).atan() / (2.0 * PAI);
    if ls < 0.0 {
        ls += 0.5;
    }
    let mut lo = if x == 0.0 { 0.5 } else { (-zo / x).atan() / (2.0 * PAI) };
    if lo < 0.0 {
        lo += 0.5;
    }
    let s = calc_swr(z.r, z.j, zi_ref, 0.0);
    (zi, x, z, ls, lo, s)
}

/// `CalcSM` and the solutions `SetValueItem` shows. The original shows the
/// first and the last position found; this returns every one.
pub fn stub_match(input: &StubMatchInput) -> Result<StubMatch, CalcError> {
    let f = positive("frequency", input.frequency_mhz)?;
    let zl = impedance("load", input.load)?;
    let zi_ref = positive("reference impedance", input.reference_ohm)?;
    let zo = positive("cable impedance", input.cable.impedance_ohm)?;
    let cm = positive("velocity factor", input.cable.velocity_factor)?;
    let solution = |l: f64| {
        let (zs, x, z, ls, lo, s) = sm_stub(zo, zl, zi_ref, l);
        let to_cm = |wl: f64| (29980.0 / f) * cm * wl;
        let stub = (x.abs() < 1e10).then(|| MatchingStub {
            reactance_ohm: x,
            element: if x >= 0.0 {
                Element { kind: ElementKind::L, value: 0.1591549 * x / f, reactance_ohm: x }
            } else {
                Element { kind: ElementKind::C, value: -159154.9 / (f * x), reactance_ohm: x }
            },
            short_wl: ls,
            short_cm: to_cm(ls),
            open_wl: lo,
            open_cm: to_cm(lo),
        });
        StubMatchSolution { position_wl: l, position_cm: to_cm(l), impedance: zs.into(), stub, input: z.into(), swr: s }
    };
    let mut solutions = Vec::new();
    let mut as_ = 9e100;
    let mut al = NULLV;
    let mut l = 0.0;
    while l <= 0.5 {
        let mut s = sm_stub(zo, zl, zi_ref, l).5;
        if as_ < s {
            if al != NULLV && as_ < 1.5 {
                let found = solution(al);
                s = found.swr;
                solutions.push(found);
            }
            al = NULLV;
        } else {
            al = l;
        }
        as_ = s;
        l += 0.0001;
    }
    solutions.retain(|s| {
        [s.position_cm, s.impedance.re, s.impedance.im, s.input.re, s.input.im, s.swr].iter().all(|v| v.is_finite())
    });
    Ok(StubMatch { solutions })
}

// ----------------------------------------------------------- hairpin ---

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct HairpinInput {
    /// The antenna's resistance (the SWR reference R).
    pub resistance_ohm: f64,
    /// The impedance the hairpin matches to.
    pub target_ohm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Hairpin {
    /// The antenna's reactance a shunt coil matches to the target
    /// (the SWR reference jX); None when the target is below the resistance.
    pub reactance_ohm: Option<f64>,
}

/// `AutojXClick`: jX = −R √(Z / R − 1).
pub fn hairpin(input: &HairpinInput) -> Result<Hairpin, CalcError> {
    let r = positive("resistance", input.resistance_ohm)?;
    let mro = positive("target impedance", input.target_ohm)?;
    let reactance_ohm = if mro > r {
        Some(-(((mro / r) - 1.0).sqrt() * r))
    } else if mro == r {
        Some(0.0)
    } else {
        None
    };
    Ok(Hairpin { reactance_ohm })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tank_resonates() {
        let t = tank(&TankInput::Inductance { frequency_mhz: 14.0, inductance_uh: 1.0 }).unwrap();
        let back =
            tank(&TankInput::Resonance { inductance_uh: 1.0, capacitance_pf: t.capacitance_pf.unwrap() }).unwrap();
        assert!((back.frequency_mhz.unwrap() - 14.0).abs() < 1e-9);
    }

    #[test]
    fn a_reactance_of_zero_leaves_l_and_c_blank() {
        let t = tank(&TankInput::Reactance { frequency_mhz: 7.0, reactance_ohm: 0.0 }).unwrap();
        assert_eq!((t.inductance_uh, t.capacitance_pf), (None, None));
    }

    #[test]
    fn the_l_network_matches() {
        for (r, x) in [(12.5, -30.0), (25.0, 25.0), (200.0, 150.0), (1000.0, -500.0)] {
            for circuit in [LcCircuit::ShuntC, LcCircuit::SeriesC] {
                let m = lc_match(&LcMatchInput {
                    frequency_mhz: 7.0,
                    line_ohm: 50.0,
                    antenna: Complex { re: r, im: x },
                    circuit,
                })
                .unwrap();
                let jx = |e: Option<Element>| e.map_or(0.0, |e| e.reactance_ohm);
                let z = Clx::new(r, x);
                let par = |z: Clx, x: f64| {
                    Clx::real(1.0).div(Clx::real(1.0).div(z).add(Clx::real(1.0).div(Clx::new(0.0, x))))
                };
                let zin = match m.shunt_side {
                    ShuntSide::Line => par(z.add(Clx::new(0.0, jx(m.series))), jx(m.shunt)),
                    ShuntSide::Antenna => par(z, jx(m.shunt)).add(Clx::new(0.0, jx(m.series))),
                };
                assert!((zin.r - 50.0).abs() < 1e-6 && zin.j.abs() < 1e-6, "{r} {x} {circuit:?}: {zin:?}");
            }
        }
    }

    #[test]
    fn a_quarter_wave_line_inverts() {
        let l = line(&LineInput {
            impedance_ohm: 50.0,
            length_wl: 0.25,
            loss_db: 0.0,
            known: LineEnd::Load { impedance: Complex { re: 100.0, im: 0.0 } },
        })
        .unwrap();
        let z = l.input.unwrap();
        assert!((z.re - 25.0).abs() < 1e-9 && z.im.abs() < 1e-9, "{z:?}");
    }

    #[test]
    fn the_hairpin_reactance_matches() {
        let h = hairpin(&HairpinInput { resistance_ohm: 25.0, target_ohm: 50.0 }).unwrap();
        assert_eq!(h.reactance_ohm, Some(-25.0));
    }

    #[test]
    fn inputs_are_checked() {
        assert!(tank(&TankInput::Reactance { frequency_mhz: 0.0, reactance_ohm: 50.0 }).is_err());
        assert!(tank(&TankInput::Inductance { frequency_mhz: 7.0, inductance_uh: f64::NAN }).is_err());
        assert!(
            lc_match(&LcMatchInput {
                frequency_mhz: 7.0,
                line_ohm: 50.0,
                antenna: Complex { re: 0.0, im: 0.0 },
                circuit: LcCircuit::ShuntC
            })
            .is_err()
        );
    }
}
