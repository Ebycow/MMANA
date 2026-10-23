//! What is measured of one antenna, in the quantities a builder uses.

use anyhow::{Result, bail};
use mmana_core::resonance::{ResonanceRange, find_resonance};
use mmana_core::{CalcOptions, GroundKind, Project, Segmentation, WireMaterial, solve};

/// The figures of one antenna at its resonance (at the design frequency
/// when it has none from half to twice it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub resonance_mhz: Option<f64>,
    /// Feed resistance at the resonance.
    pub r_ohm: f64,
    /// Highest total gain over the sphere (dBi).
    pub gain_dbi: f64,
    /// Average gain test: the power radiated over the sphere ÷ the input
    /// power, in dB (0 for a correct model). Only in free space with
    /// lossless wires and no loads, where nothing else takes power.
    pub agt_db: Option<f64>,
}

/// Angular step of the sphere the gain is integrated over (degrees).
#[derive(Debug, Clone, Copy)]
pub struct Grid(pub f64);

impl Grid {
    pub const FINE: Grid = Grid(2.0);
    pub const COARSE: Grid = Grid(10.0);
}

pub fn measure(p: &Project, grid: Grid) -> Result<Metrics> {
    let opts = CalcOptions::default();
    let found = find_resonance(p, &opts, &ResonanceRange::default())?;
    let resonance_mhz = found.resonance.as_ref().map(|r| r.frequency_mhz);
    let f = resonance_mhz.unwrap_or(p.antenna.frequency_mhz);
    let solved = solve(p, &CalcOptions { frequency_mhz: Some(f), ..opts })?;
    let r_ohm = solved.feed().0.re;
    // cell centres of a `step` grid over the whole sphere
    let step = grid.0;
    let nz = (180.0 / step).round() as i32;
    let na = (360.0 / step).round() as i32;
    let samples = solved.far_field((step / 2.0, step, nz), (0.0, step, na))?;
    let gain_dbi = samples.iter().map(|s| s.total_db).fold(f64::MIN, f64::max);
    let lossless = p.environment.ground == GroundKind::FreeSpace
        && p.environment.wire_material == WireMaterial::Lossless
        && (p.antenna.loads.is_empty() || !p.antenna.loads_enabled);
    let agt_db = lossless.then(|| {
        let d = step.to_radians();
        let total: f64 = samples
            .iter()
            .map(|s| if s.total_db <= -999.0 { 0.0 } else { 10f64.powf(s.total_db / 10.0) } * s.zenith.to_radians().sin() * d * d)
            .sum();
        10.0 * (total / (4.0 * std::f64::consts::PI)).log10()
    });
    if !(r_ohm.is_finite() && gain_dbi.is_finite()) {
        bail!("the figures are not finite");
    }
    Ok(Metrics { resonance_mhz, r_ohm, gain_dbi, agt_db })
}

/// The same antenna with every fixed segment count multiplied by `k` (the
/// other segmentations are left as they are).
pub fn refined(p: &Project, k: i32) -> Project {
    let mut q = p.clone();
    for w in &mut q.antenna.wires {
        if let Segmentation::Fixed { count } = &mut w.segmentation {
            *count *= k;
        }
    }
    q
}

/// How far `a` is from `b`, in percent of `b`.
pub fn percent(a: f64, b: f64) -> f64 {
    100.0 * (a - b) / b
}
