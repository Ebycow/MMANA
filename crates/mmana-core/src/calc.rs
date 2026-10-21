//! High-level calculation entry points: one frequency, frequency sweeps,
//! custom far-field scans and near fields.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::engine::antdef::AntDef;
use crate::engine::{EngineConfig, FarFieldSample, MediumDef, Mininec, NULLV, NearFieldSample, NearFieldSpec};
use crate::error::CalcError;
use crate::model::{Antenna, GroundKind, Point3, Project, Segmentation};
use crate::pattern::{self, PatternState};

/// Angular resolution of the stored pattern cuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum PatternResolution {
    /// 1° (MMANA's normal calculation).
    #[default]
    Fine,
    /// 2° (used by MMANA's optimiser; about twice as fast).
    Coarse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct CalcOptions {
    /// Calculation frequency in MHz; defaults to the antenna's design frequency.
    pub frequency_mhz: Option<f64>,
    pub pattern_resolution: PatternResolution,
    /// In free space, take the horizontal cut at 0° elevation instead of at
    /// the vertical peak (MMANA option "FixFreeAngle", on by default).
    pub fix_free_space_elevation: bool,
    /// Take the horizontal cut at this elevation instead of the computed one.
    pub horizontal_cut_elevation_deg: Option<f64>,
    /// Upper bound on the number of pulses (unknowns).
    pub max_pulses: usize,
}

impl Default for CalcOptions {
    fn default() -> Self {
        Self {
            frequency_mhz: None,
            pattern_resolution: PatternResolution::Fine,
            fix_free_space_elevation: true,
            horizontal_cut_elevation_deg: None,
            max_pulses: 4096,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl From<(f64, f64)> for Complex {
    fn from((re, im): (f64, f64)) -> Self {
        Self { re, im }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SourceResult {
    pub pulse: usize,
    pub voltage: Complex,
    pub current: Complex,
    pub impedance: Complex,
    pub power_w: f64,
    pub swr: f64,
}

/// Gain in dBi per direction for one cut, split by polarisation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PatternCut {
    /// Angle of sample 0 and the step between samples (degrees).
    pub start_deg: f64,
    pub step_deg: f64,
    pub vertical_pol: Vec<f32>,
    pub horizontal_pol: Vec<f32>,
    pub total: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Patterns {
    /// Azimuth cut (sample i = azimuth i·step) at `horizontalElevationDeg`.
    pub horizontal: PatternCut,
    pub horizontal_elevation_deg: f32,
    /// Elevation cut in the X-Z plane: sample 0 is the horizon behind the
    /// antenna (azimuth 180°), the middle sample the zenith, the last sample
    /// the horizon in front (azimuth 0°).
    pub vertical: PatternCut,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PulseCurrent {
    pub pulse: usize,
    /// 1-based index of the defining wire.
    pub wire: usize,
    /// Position of the pulse (metres, antenna coordinates without the height).
    pub position: Point3,
    pub current: Complex,
}

/// A wire as subdivided for the calculation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SegmentedWire {
    pub wire: usize,
    pub start: Point3,
    pub end: Point3,
    pub radius: f64,
    pub segments: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CalcResult {
    pub frequency_mhz: f64,
    pub wavelength_m: f64,
    pub pulse_count: usize,
    pub sources: Vec<SourceResult>,
    /// Impedance and SWR at the first source.
    pub impedance: Complex,
    pub swr: f64,
    pub gain_dbi: f32,
    pub fb_db: f32,
    /// Take-off angle of the main lobe (degrees above the horizon).
    pub elevation_deg: f32,
    pub max_horizontal_pol_dbi: f32,
    pub max_vertical_pol_dbi: f32,
    pub total_power_w: f64,
    pub patterns: Patterns,
    pub currents: Vec<PulseCurrent>,
    /// Current magnitude per pulse as MMANA plots it: signed by the dominant
    /// direction; index = pulse − 1.
    pub current_display: Vec<f32>,
    pub segmented_wires: Vec<SegmentedWire>,
    pub warnings: Vec<String>,
}

/// A prepared and solved model; further scans reuse the solution.
pub struct Solved {
    ap: AntDef,
    engine: Mininec,
    warnings: Vec<String>,
}

fn engine_config(project: &Project, opts: &CalcOptions) -> EngineConfig {
    let env = &project.environment;
    let unset = |v: Option<f64>, default: f64| match v {
        Some(v) if v != NULLV => v,
        _ => default,
    };
    EngineConfig {
        ground: env.ground,
        height: env.height,
        media: env
            .real_ground
            .media
            .iter()
            .map(|m| MediumDef {
                rel: m.permittivity,
                cond: m.conductivity,
                boundary: unset(m.boundary, 1000000.0),
                height: unset(m.height, 0.0),
            })
            .collect(),
        screen: env.real_ground.screen.map(|s| (s.radials, s.radial_radius)),
        wire_material: env.wire_material,
        swr_ro: env.swr_reference.r,
        swr_jxo: env.swr_reference.x,
        max_pulses: opts.max_pulses.min(crate::engine::PMAX),
    }
}

/// Rejects segmentation settings the tapering loops never finish with
/// (`SC <= 1`, `DM2 <= 0`). MMANA's dialogs and the .maa reader keep them in
/// range; project JSON and API requests do not.
fn check_segmentation(ant: &Antenna) -> Result<(), CalcError> {
    let s = &ant.auto_segmentation;
    if s.dm1 < 1 || s.dm2 < 1 {
        return Err(CalcError::InvalidAutoSegmentation("DM1 and DM2 must be at least 1"));
    }
    if !(s.sc > 1.0 && s.sc <= 3.0) {
        return Err(CalcError::InvalidAutoSegmentation("SC must be greater than 1 and at most 3"));
    }
    if s.ec < 1 {
        return Err(CalcError::InvalidAutoSegmentation("EC must be at least 1"));
    }
    for (i, w) in ant.wires.iter().enumerate() {
        // wires with R = 0 are not expanded
        if w.radius == 0.0 {
            continue;
        }
        // the SEG code packs each DM into 15 bits; a bare DM1 above 10000
        // would be read back as a packed pair
        let (values, max) = match w.segmentation {
            // a count below 1 would be read as SEG 0 or a tapering code
            // (-2147483248 is DM1 = 400 with DM2 = 0, which never finishes)
            Segmentation::Fixed { count } if count < 1 => {
                return Err(CalcError::InvalidSegmentCount { wire: i + 1, count });
            }
            Segmentation::TaperCustom { dm1, dm2: Some(dm2) } => ([dm1, dm2], 0x7fff),
            Segmentation::TaperCustom { dm1, dm2: None } => ([dm1, dm1], 10000),
            _ => continue,
        };
        if values.iter().any(|v| !(1..=max).contains(&v.unsigned_abs())) {
            return Err(CalcError::InvalidWireSegmentation { wire: i + 1, max: max as i32 });
        }
    }
    Ok(())
}

/// Rejects two wires with the same ends (in either direction), such as a
/// copy left where it was made. Their impedance matrix is singular and the
/// original carries on with NaN currents. Wires with R = 0 are not
/// calculated, so they may lie anywhere.
fn check_overlap(ant: &Antenna) -> Result<(), CalcError> {
    let same = |a: &Point3, b: &Point3| a.x == b.x && a.y == b.y && a.z == b.z;
    for (i, a) in ant.wires.iter().enumerate().filter(|(_, w)| w.radius != 0.0) {
        for (j, b) in ant.wires.iter().enumerate().skip(i + 1).filter(|(_, w)| w.radius != 0.0) {
            if (same(&a.start, &b.start) && same(&a.end, &b.end)) || (same(&a.start, &b.end) && same(&a.end, &b.start))
            {
                return Err(CalcError::OverlappingWires { first: i + 1, second: j + 1 });
            }
        }
    }
    Ok(())
}

/// Builds the geometry and solves for the currents at one frequency.
pub fn solve(project: &Project, opts: &CalcOptions) -> Result<Solved, CalcError> {
    let ant = &project.antenna;
    let cfq = opts.frequency_mhz.unwrap_or(ant.frequency_mhz);
    if !(cfq.is_finite() && cfq > 0.0) {
        return Err(CalcError::InvalidFrequency(cfq));
    }
    if ant.wires.is_empty() {
        return Err(CalcError::NoWires);
    }
    check_segmentation(ant)?;
    check_overlap(ant)?;
    let mut ap = AntDef::from_model(ant, cfq);
    let cfg = engine_config(project, opts);
    let mut engine = Mininec::new(&mut ap, &cfg)?;
    engine.solve();
    // other degenerate geometry (stacked copies 0 m apart...) ends the same way
    if engine.solutions.iter().any(|s| !(s.current.0.is_finite() && s.current.1.is_finite())) {
        return Err(CalcError::NoFiniteSolution);
    }
    let mut warnings = Vec::new();
    if project.environment.ground != GroundKind::FreeSpace && ap.min_z + project.environment.height < 0.0 {
        warnings.push("some wires are below ground".to_owned());
    }
    Ok(Solved { ap, engine, warnings })
}

impl Solved {
    /// The standard MMANA result: impedance, gain, F/B, patterns, currents.
    pub fn result(&self, project: &Project, opts: &CalcOptions) -> CalcResult {
        let e = &self.engine;
        let ptn_f = match opts.pattern_resolution {
            PatternResolution::Fine => 1,
            PatternResolution::Coarse => 2,
        };
        let env = &project.environment;
        let fix = env.ground == GroundKind::FreeSpace && opts.fix_free_space_elevation;
        let fbr = env.fbr_code();
        let mut p = pattern::scan(e, ptn_f, fix, fbr);
        if let Some(el) = opts.horizontal_cut_elevation_deg {
            pattern::rescan_horizontal(e, &mut p, el, fbr);
        }
        let first = e.solutions[0];
        CalcResult {
            frequency_mhz: e.f,
            wavelength_m: e.wavelength,
            pulse_count: e.n,
            sources: e
                .solutions
                .iter()
                .map(|s| SourceResult {
                    pulse: s.pulse,
                    voltage: s.voltage.into(),
                    current: s.current.into(),
                    impedance: s.impedance.into(),
                    power_w: s.power,
                    swr: s.swr,
                })
                .collect(),
            impedance: first.impedance.into(),
            swr: first.swr,
            gain_dbi: p.max_g,
            fb_db: p.fb,
            elevation_deg: p.elv,
            max_horizontal_pol_dbi: p.max_hp,
            max_vertical_pol_dbi: p.max_vp,
            total_power_w: e.pwr,
            patterns: patterns(&p),
            currents: self.currents(),
            current_display: e.current_distribution(&self.ap),
            segmented_wires: self
                .ap
                .wz
                .iter()
                .map(|w| SegmentedWire {
                    wire: w.wno + 1,
                    start: Point3::new(w.x1, w.y1, w.z1),
                    end: Point3::new(w.x2, w.y2, w.z2),
                    radius: w.r,
                    segments: w.seg,
                })
                .collect(),
            warnings: self.warnings.clone(),
        }
    }

    fn currents(&self) -> Vec<PulseCurrent> {
        let e = &self.engine;
        (1..=e.n)
            .map(|i| {
                let seg = (2 * e.wi[i] - 1 + i as i32) as usize;
                let (x, y, z) = e.seg_pos(seg);
                let wz = (e.wi[i] as usize).saturating_sub(1);
                PulseCurrent {
                    pulse: i,
                    wire: self.ap.wz.get(wz).map_or(0, |w| w.wno + 1),
                    position: Point3::new(x, y, z),
                    current: (e.cr[i], e.cim[i]).into(),
                }
            })
            .collect()
    }

    /// Arbitrary far-field scan (MMANA's "far field file"): `nz` zenith
    /// angles from `zenith_start` for each of `na` azimuths.
    pub fn far_field(&self, zenith: (f64, f64, i32), azimuth: (f64, f64, i32)) -> Vec<FarFieldSample> {
        let (za, mut zc, mut nz) = zenith;
        let (aa, mut ac, mut na) = azimuth;
        if zc == 0.0 {
            nz = 1;
            zc = 0.0;
        }
        if ac == 0.0 {
            na = 1;
            ac = 0.0;
        }
        let mut out = Vec::new();
        self.engine.far_field(za, zc, nz, aa, ac, na, |s| out.push(s));
        out
    }

    pub fn near_field(&self, spec: &NearFieldSpec) -> Vec<NearFieldSample> {
        self.engine.near_field(spec)
    }

    /// `Str2PlusNo` on the calculated (stacked) model: the 1-based pulse a
    /// name such as `w1c` refers to, or 0.
    pub(crate) fn pulse_no(&self, name: &str) -> i32 {
        self.ap.pulse_no(name)
    }
}

fn patterns(p: &PatternState) -> Patterns {
    let pf = p.ptn_f as usize;
    let nh = 360 / pf;
    let nv = 180 / pf + 1;
    Patterns {
        horizontal: PatternCut {
            start_deg: 0.0,
            step_deg: pf as f64,
            vertical_pol: p.ptn_h[0][..nh].to_vec(),
            horizontal_pol: p.ptn_h[1][..nh].to_vec(),
            total: p.ptn_h[2][..nh].to_vec(),
        },
        horizontal_elevation_deg: p.elv_h_ptn,
        vertical: PatternCut {
            start_deg: 0.0,
            step_deg: pf as f64,
            vertical_pol: p.ptn_v[0][..nv].to_vec(),
            horizontal_pol: p.ptn_v[1][..nv].to_vec(),
            total: p.ptn_v[2][..nv].to_vec(),
        },
    }
}

/// Convenience: solve and produce the standard result.
pub fn calculate(project: &Project, opts: &CalcOptions) -> Result<CalcResult, CalcError> {
    Ok(solve(project, opts)?.result(project, opts))
}

/// One point of a frequency sweep.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SweepPoint {
    pub frequency_mhz: f64,
    pub impedance: Complex,
    pub swr: f64,
    pub gain_dbi: f32,
    pub fb_db: f32,
    pub elevation_deg: f32,
}

/// Evaluates `points` frequencies evenly spaced from `start` to `stop` (MHz).
pub fn sweep(
    project: &Project,
    opts: &CalcOptions,
    start: f64,
    stop: f64,
    points: usize,
) -> Result<Vec<SweepPoint>, CalcError> {
    let points = points.max(1);
    // frequencies are independent: solve them in parallel
    crate::par::map(points, |i| {
        let f = if points == 1 { start } else { start + (stop - start) * i as f64 / (points - 1) as f64 };
        let o = CalcOptions { frequency_mhz: Some(f), ..opts.clone() };
        let r = calculate(project, &o)?;
        Ok(SweepPoint {
            frequency_mhz: f,
            impedance: r.impedance,
            swr: r.swr,
            gain_dbi: r.gain_dbi,
            fb_db: r.fb_db,
            elevation_deg: r.elevation_deg,
        })
    })
    .into_iter()
    .collect()
}
