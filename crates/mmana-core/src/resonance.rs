//! The resonant frequency (where the reactance at the first source is 0 and
//! rising) and the series circuit the antenna behaves like near it: the
//! 共振周波数を計算 button and the 推測計算 of MMANA's frequency response
//! dialog (`TBwDispDlg::GetFoBtnClick`, `CalcSim` in `BwDisp.cpp`).
//!
//! The original re-centres and narrows a sweep of the dialog until the
//! reactance is under 0.1 Ω or the estimate moves less than 0.1 %, rounding
//! the centre to 1 kHz. Here the search steps towards the resonance until
//! the reactance changes sign, then narrows the interval; it stops at the
//! same 0.1 Ω, so its frequency is at least as close to X = 0 as the
//! original's.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::calc::{CalcOptions, Complex, solve};
use crate::error::CalcError;
use crate::model::Project;

/// The reactance a resonance is found at (Ω), as the original stops.
pub const RESONANCE_REACTANCE: f64 = 0.1;

/// Calculations one search makes at most.
pub const MAX_RESONANCE_CALCULATIONS: usize = 60;

/// The interval is narrowed no further than this fraction of the frequency.
const RESOLUTION: f64 = 1e-9;

/// The first step from the start frequency, as a fraction of it.
const FIRST_STEP: f64 = 0.005;

/// How far a step may go before the resonance is bracketed (a factor).
const MAX_STEP: f64 = 1.25;

/// The antenna near a resonance as a series R-L-C circuit, fitted to the
/// reactance at two frequencies (MMANA's 推測計算).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SeriesCircuit {
    pub inductance_uh: f64,
    pub capacitance_pf: f64,
    /// sqrt(L / C) / R, with R at the frequency closer to resonance.
    pub q: f64,
    pub resonance_mhz: f64,
    /// Resonance / Q (the bandwidth of the circuit between its −3 dB points).
    pub bandwidth_khz: f64,
}

/// Fits a series R-L-C circuit to the impedance at two frequencies (MHz),
/// `a` the one closer to resonance (`CalcSim`). None when the two cannot
/// be told apart, or the reactance does not rise with frequency as a series
/// circuit's does (the original took the absolute values then).
pub fn series_circuit(a: (f64, Complex), b: (f64, Complex)) -> Option<SeriesCircuit> {
    let pi2 = 2.0 * std::f64::consts::PI;
    let (fa, xa, ra) = (a.0 * 1e6, a.1.im, a.1.re);
    let (fb, xb) = (b.0 * 1e6, b.1.im);
    if fa == fb {
        return None;
    }
    let l = ((fa * xa) - (fb * xb)) / ((fa * fa - fb * fb) * pi2);
    let x = pi2 * fa * l - xa;
    if !(l > 0.0 && x > 0.0 && ra > 0.0) {
        return None;
    }
    let c = 1.0 / (pi2 * fa * x);
    let q = (l / c).sqrt() / ra;
    let resonance_mhz = 1e-6 / (pi2 * (l * c).sqrt());
    let circuit = SeriesCircuit {
        inductance_uh: l * 1e6,
        capacitance_pf: c * 1e12,
        q,
        resonance_mhz,
        bandwidth_khz: resonance_mhz * 1e3 / q,
    };
    [circuit.inductance_uh, circuit.capacitance_pf, q, resonance_mhz, circuit.bandwidth_khz]
        .iter()
        .all(|v| v.is_finite())
        .then_some(circuit)
}

/// A resonance found.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Resonance {
    pub frequency_mhz: f64,
    /// At the first source.
    pub impedance: Complex,
    pub swr: f64,
    /// |X| < [`RESONANCE_REACTANCE`]. False when the reactance jumps across 0
    /// instead (the automatic segmentation changes with the frequency, and a
    /// segment more or less moves it): the frequency is then where it jumps.
    pub converged: bool,
    /// Fitted to the two frequencies calculated closest to the resonance.
    pub circuit: Option<SeriesCircuit>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ResonanceSearch {
    /// None when there is no resonance from `minMhz` to `maxMhz`, or the
    /// search did not reach it within [`MAX_RESONANCE_CALCULATIONS`].
    pub resonance: Option<Resonance>,
    pub min_mhz: f64,
    pub max_mhz: f64,
    pub calculations: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct ResonanceRange {
    /// Where the search starts (MHz); the calculation frequency by default.
    pub start_mhz: Option<f64>,
    /// The lowest frequency searched (MHz); half the start by default (the original's limit).
    pub min_mhz: Option<f64>,
    /// The highest frequency searched (MHz); twice the start by default (the original's limit).
    pub max_mhz: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    f: f64,
    z: Complex,
    swr: f64,
}

/// Finds the resonance nearest to the start frequency in the direction the
/// reactance there points to (down when inductive, up when capacitive).
pub fn find_resonance(
    project: &Project,
    opts: &CalcOptions,
    range: &ResonanceRange,
) -> Result<ResonanceSearch, CalcError> {
    let start = range.start_mhz.or(opts.frequency_mhz).unwrap_or(project.antenna.frequency_mhz);
    if !(start.is_finite() && start > 0.0) {
        return Err(CalcError::InvalidFrequency(start));
    }
    let lo = range.min_mhz.unwrap_or(start / 2.0);
    let hi = range.max_mhz.unwrap_or(start * 2.0);
    if !(lo.is_finite() && lo > 0.0 && hi.is_finite() && lo <= start && start <= hi) {
        return Err(CalcError::OutOfRange {
            field: "resonance search range".into(),
            rule: "positive, around the start frequency",
            value: if lo <= start { hi } else { lo },
        });
    }
    let mut samples: Vec<Sample> = Vec::new();
    let calc = |f: f64, samples: &mut Vec<Sample>| -> Result<Sample, CalcError> {
        let (z, swr) = solve(project, &CalcOptions { frequency_mhz: Some(f), ..opts.clone() })?.feed();
        let s = Sample { f, z, swr };
        samples.push(s);
        Ok(s)
    };
    let done = |samples: &[Sample], converged: bool, lo: f64, hi: f64| {
        let best = samples.iter().min_by(|a, b| a.z.im.abs().total_cmp(&b.z.im.abs())).copied();
        ResonanceSearch {
            resonance: best.map(|b| resonance(samples, b, converged)),
            min_mhz: lo,
            max_mhz: hi,
            calculations: samples.len(),
        }
    };
    let found = |s: Sample| s.z.im.abs() < RESONANCE_REACTANCE;

    let first = calc(start, &mut samples)?;
    if found(first) {
        return Ok(done(&samples, true, lo, hi));
    }
    // a series resonance lies above while the antenna is capacitive
    let up = first.z.im < 0.0;
    let limit = if up { hi } else { lo };
    let mut last = first;
    let mut next = if up { start * (1.0 + FIRST_STEP) } else { start / (1.0 + FIRST_STEP) };
    // step towards it until the reactance changes sign
    let (mut below, mut above) = loop {
        next = if up { next.min(limit) } else { next.max(limit) };
        if next == last.f || samples.len() >= MAX_RESONANCE_CALCULATIONS {
            return Ok(ResonanceSearch { resonance: None, ..done(&samples, false, lo, hi) });
        }
        let s = calc(next, &mut samples)?;
        if found(s) {
            return Ok(done(&samples, true, lo, hi));
        }
        if (s.z.im < 0.0) != (last.z.im < 0.0) {
            break if up { (last, s) } else { (s, last) };
        }
        // the series circuit through the last two points says where X = 0
        let guess = series_circuit((s.f, s.z), (last.f, last.z)).map(|c| c.resonance_mhz);
        let (near, far) = if up { (s.f * 1.001, s.f * MAX_STEP) } else { (s.f / MAX_STEP, s.f / 1.001) };
        next = match guess {
            Some(g) if (g > s.f) == up => g.clamp(near.min(far), near.max(far)),
            _ => {
                if up {
                    far
                } else {
                    near
                }
            }
        };
        last = s;
    };
    // X < 0 at `below`, > 0 at `above`: narrow the interval by the fitted
    // circuit, or by halving when the last step did not halve it
    let mut previous = f64::INFINITY;
    while samples.len() < MAX_RESONANCE_CALCULATIONS {
        let width = above.f - below.f;
        if width <= RESOLUTION * above.f {
            return Ok(done(&samples, false, lo, hi));
        }
        let guess = series_circuit((below.f, below.z), (above.f, above.z)).map(|c| c.resonance_mhz);
        let f = match guess {
            Some(g) if g > below.f && g < above.f && width <= 0.5 * previous => g,
            _ => 0.5 * (below.f + above.f),
        };
        previous = width;
        let s = calc(f, &mut samples)?;
        if found(s) {
            return Ok(done(&samples, true, lo, hi));
        }
        if s.z.im < 0.0 {
            below = s;
        } else {
            above = s;
        }
    }
    Ok(done(&samples, false, lo, hi))
}

/// The resonance at `best`, with the circuit fitted to it and the other
/// sample closest to X = 0.
fn resonance(samples: &[Sample], best: Sample, converged: bool) -> Resonance {
    let other = samples.iter().filter(|s| s.f != best.f).min_by(|a, b| a.z.im.abs().total_cmp(&b.z.im.abs())).copied();
    Resonance {
        frequency_mhz: best.f,
        impedance: best.z,
        swr: best.swr,
        converged,
        circuit: other.and_then(|o| series_circuit((best.f, best.z), (o.f, o.z))),
    }
}
