//! High-level calculation entry points: one frequency, frequency sweeps,
//! custom far-field scans and near fields.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::engine::antdef::AntDef;
use crate::engine::{
    AxisRange, EngineConfig, FarFieldSample, MediumDef, Mininec, NULLV, NearFieldSample, NearFieldSpec, SourceSolution,
};
use crate::error::CalcError;
use crate::model::{GroundKind, Point3, Project, Stack, StackDirection, Wire};
use crate::pattern::{self, PatternState};
use crate::validate::{self, Scope};

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
    /// direction; index = pulse − 1. Like the original's list it ends at the
    /// pulse set last, so it can be shorter than `pulseCount` (a loop's
    /// closing pulse is left out), and a pulse it gives no value is
    /// 1.2345e-37. `currents` has every pulse.
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

/// Checks the input and builds the definition the engine works on.
fn prepare(project: &Project, opts: &CalcOptions, scope: Scope) -> Result<(AntDef, EngineConfig), CalcError> {
    let cfq = validate::project(project, opts, scope)?;
    Ok((AntDef::from_model(&project.antenna, cfq), engine_config(project, opts)))
}

/// Builds the geometry and solves for the currents at one frequency.
pub fn solve(project: &Project, opts: &CalcOptions) -> Result<Solved, CalcError> {
    let (mut ap, cfg) = prepare(project, opts, Scope::Solve)?;
    let mut engine = Mininec::new(&mut ap, &cfg)?;
    engine.solve();
    // other degenerate geometry (stacked copies 0 m apart...) ends the same way;
    // the feed figures are checked too, as a source with no current has none
    let finite = |s: &SourceSolution| {
        [s.current.0, s.current.1, s.impedance.0, s.impedance.1, s.swr].iter().all(|v| v.is_finite())
    };
    if !engine.solutions.iter().all(finite) {
        return Err(CalcError::NoFiniteSolution);
    }
    let mut warnings = Vec::new();
    // (an end an ulp below the ground is on it, see `GROUND_CONTACT`)
    let longest = ap.wz.iter().map(|w| (w.x2 - w.x1).hypot(w.y2 - w.y1).hypot(w.z2 - w.z1)).fold(0.0, f64::max);
    if project.environment.ground != GroundKind::FreeSpace
        && ap.min_z + project.environment.height < -crate::engine::GROUND_CONTACT * longest
    {
        warnings.push("some wires are below ground".to_owned());
    }
    // a passive antenna always takes power; less than none means the method
    // has broken down (each source alone may still take some), and every
    // gain is then -999
    let apparent =
        engine.solutions.iter().map(|s| s.voltage.0.hypot(s.voltage.1) * s.current.0.hypot(s.current.1) / 2.0);
    let apparent: f64 = apparent.sum();
    if engine.pwr <= 0.0 {
        warnings.push(format!(
            "the input power is not positive ({:e} W): the model is outside what MININEC can calculate, \
             such as wires meeting at a sharp angle or with very different radii",
            engine.pwr
        ));
    } else if !takes_power(engine.pwr, apparent) {
        // the matrix is f32: a feed resistance under a millionth of |Z| is
        // rounding, and so is the gain divided by the power it gives
        warnings.push(format!(
            "the input power is negligible ({:e} W of {apparent:e} VA): the feed resistance is within the \
             rounding of the calculation, so the gain and the other figures that depend on the power are not \
             reliable",
            engine.pwr
        ));
    }
    if let Some((wire, segments)) = most_divided_conductor(&ap) {
        warnings.push(format!(
            "wire {wire} and the wires joined to it are divided into {segments} segments: beyond about \
             {MAX_STEADY_SEGMENTS} on one conductor the results drift as segments are added (the reactance \
             by several ohms), so compare with fewer segments"
        ));
    }
    Ok(Solved { ap, engine, warnings })
}

/// Whether an antenna taking `power` W of `apparent` VA takes power the
/// calculation can tell from rounding (a millionth of it and more).
pub(crate) fn takes_power(power: f64, apparent: f64) -> bool {
    power > 1e-6 * apparent
}

/// Segments one conductor can be divided into before MININEC's results start
/// to drift. Beyond it the error grows with each segment added, whatever
/// their length in wavelengths or the wire radius (the original gives the
/// same values): a half-wave dipole in 320 segments is about 6 Ω off in X,
/// in 640 about 25 Ω. Separate conductors do not add up.
const MAX_STEADY_SEGMENTS: i64 = 250;

/// The conductor (segment wires joined end to end, as `connect` joins them)
/// with the most segments, when it has more than [`MAX_STEADY_SEGMENTS`]:
/// the 1-based number of a wire of it as defined (in a stacked copy, the
/// wire it copies) and its segment count.
fn most_divided_conductor(ap: &AntDef) -> Option<(usize, i64)> {
    let wz = &ap.wz;
    let mut root: Vec<usize> = (0..wz.len()).collect();
    fn find(root: &mut [usize], mut i: usize) -> usize {
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    // ends compare with ==, so -0.0 is 0.0 (adding 0.0 makes it so)
    let key = |x: f64, y: f64, z: f64| [(x + 0.0).to_bits(), (y + 0.0).to_bits(), (z + 0.0).to_bits()];
    let mut first_at = std::collections::HashMap::new();
    for (i, w) in wz.iter().enumerate() {
        for end in [key(w.x1, w.y1, w.z1), key(w.x2, w.y2, w.z2)] {
            let j = *first_at.entry(end).or_insert(i);
            let (a, b) = (find(&mut root, i), find(&mut root, j));
            root[a] = b;
        }
    }
    let mut segments = vec![0i64; wz.len()];
    for (i, w) in wz.iter().enumerate() {
        let r = find(&mut root, i);
        segments[r] += i64::from(w.seg.max(0));
    }
    let (r, &most) = segments.iter().enumerate().max_by_key(|(_, s)| **s)?;
    (most > MAX_STEADY_SEGMENTS).then(|| {
        let first = (0..wz.len()).find(|&i| find(&mut root, i) == r).unwrap_or(r);
        (wz[first].wno % ap.base_wires.max(1) + 1, most)
    })
}

impl Solved {
    /// The standard MMANA result: impedance, gain, F/B, patterns, currents.
    /// Fails only when a value of it cannot be represented
    /// ([`CalcError::NotRepresentable`]).
    pub fn result(&self, project: &Project, opts: &CalcOptions) -> Result<CalcResult, CalcError> {
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
        let r = CalcResult {
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
        };
        validate::result(&r)?;
        Ok(r)
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
    pub fn far_field(
        &self,
        zenith: (f64, f64, i32),
        azimuth: (f64, f64, i32),
    ) -> Result<Vec<FarFieldSample>, CalcError> {
        let axis = |(start, step, count)| AxisRange { start, step, count };
        validate::far_field(&axis(zenith), &axis(azimuth))?;
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
        validate::far_samples(&out)?;
        Ok(out)
    }

    pub fn near_field(&self, spec: &NearFieldSpec) -> Result<Vec<NearFieldSample>, CalcError> {
        validate::near_field(spec)?;
        // the field is scaled by √(power / input power)
        if self.engine.pwr <= 0.0 {
            return Err(CalcError::NonPositivePower { power: self.engine.pwr });
        }
        let samples = self.engine.near_field(spec);
        validate::near_samples(&samples)?;
        Ok(samples)
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
    solve(project, opts)?.result(project, opts)
}

/// A pulse a source or load can be placed on, with the name that addresses it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PulsePoint {
    pub pulse: usize,
    /// `w2c`, or the shorter of `w2b3` / `w2e1`.
    pub name: String,
    /// Metres, antenna coordinates without the height.
    pub position: Point3,
}

/// The pulses on one wire, from its start to its end.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct WirePulses {
    /// 1-based wire number.
    pub wire: usize,
    pub pulses: Vec<PulsePoint>,
}

/// Where the pulses of the antenna lie, for placing sources and loads, and
/// where its sources and loads are. With stacking, these are the pulses of the
/// first copy (the one the names address), moved back onto the wires as defined.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PulseLayout {
    pub pulse_count: usize,
    /// Wires with R = 0 have no pulses and are left out.
    pub wires: Vec<WirePulses>,
    /// Position of each source's pulse; null when the name does not resolve.
    pub sources: Vec<Option<Point3>>,
    pub loads: Vec<Option<Point3>>,
}

/// Segments the antenna as the calculation would, without solving it.
pub fn pulse_layout(project: &Project, opts: &CalcOptions) -> Result<PulseLayout, CalcError> {
    let ant = &project.antenna;
    // segmented stacked, as calculated: a copy that reaches the ground is
    // divided differently from the wire as defined
    let (mut ap, cfg) = prepare(project, opts, Scope::Layout)?;
    let e = Mininec::layout(&mut ap, &cfg)?;
    // names address the first copy, whose pulses come first
    let own = ap.wires[ant.wires.len()..].iter().find(|w| w.pmax > 0).map_or(e.n, |w| (w.pno - 1) as usize);
    let (dy, dz) = first_copy_offset(&ant.stack);
    let position = |p: i32| -> Option<Point3> {
        let p = usize::try_from(p).ok().filter(|p| (1..=own).contains(p))?;
        let (x, y, z) = e.seg_pos((2 * e.wi[p] - 1 + p as i32) as usize);
        Some(Point3::new(x, y - dy, z - dz))
    };
    let wires = ant
        .wires
        .iter()
        .zip(&ap.wires)
        .enumerate()
        .filter(|(_, (_, wp))| wp.pmax > 0)
        .map(|(i, (w, wp))| {
            let centre = wp.pno + wp.pmax / 2;
            let pulses = (wp.pno..=wp.pno + wp.pmax)
                .filter_map(|p| {
                    let at = position(p)?;
                    // the last name can reach the first pulse of the next wire
                    if !on_wire(&at, w) {
                        return None;
                    }
                    let (b, e) = (p - wp.pno, wp.pno + wp.pmax - p);
                    let name = match () {
                        _ if p == centre => format!("w{}c", i + 1),
                        _ if b <= e => format!("w{}b{}", i + 1, if b == 0 { String::new() } else { b.to_string() }),
                        _ => format!("w{}e{}", i + 1, if e == 0 { String::new() } else { e.to_string() }),
                    };
                    Some(PulsePoint { pulse: p as usize, name, position: at })
                })
                .collect();
            WirePulses { wire: i + 1, pulses }
        })
        .collect();
    let layout = PulseLayout {
        pulse_count: e.n,
        wires,
        sources: ap.sources.iter().take(ant.sources.len()).map(|c| position(c.plus_no)).collect(),
        loads: ap.loads.iter().take(ant.loads.len()).map(|l| position(l.plus_no)).collect(),
    };
    validate::layout(&layout)?;
    Ok(layout)
}

/// How far `AntDef::stack` moves the first copy: along Y, along Z.
fn first_copy_offset(st: &Stack) -> (f64, f64) {
    let dy = if st.horizontal > 1 { -((st.horizontal - 1) as f64 * st.horizontal_spacing) / 2.0 } else { 0.0 };
    let dz = if st.vertical > 1 {
        match st.direction {
            StackDirection::Up => 0.0,
            StackDirection::Down => -((st.vertical - 1) as f64 * st.vertical_spacing),
            StackDirection::Centered => -((st.vertical - 1) as f64 * st.vertical_spacing) / 2.0,
        }
    } else {
        0.0
    };
    (dy, dz)
}

/// Whether `p` lies on the wire, allowing for rounding in the segmentation.
fn on_wire(p: &Point3, w: &Wire) -> bool {
    let d = [w.end.x - w.start.x, w.end.y - w.start.y, w.end.z - w.start.z];
    let v = [p.x - w.start.x, p.y - w.start.y, p.z - w.start.z];
    let len2 = d.iter().map(|c| c * c).sum::<f64>();
    let t = if len2 == 0.0 { 0.0 } else { (v[0] * d[0] + v[1] * d[1] + v[2] * d[2]) / len2 };
    let off = (0..3).map(|k| (v[k] - d[k] * t.clamp(0.0, 1.0)).powi(2)).sum::<f64>();
    off <= 1e-12 * len2.max(1e-6)
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

/// Frequency of point `i` of `points` frequencies evenly spaced from `start`
/// to `stop` (MHz).
pub fn sweep_frequency(start: f64, stop: f64, points: usize, i: usize) -> f64 {
    if points <= 1 { start } else { start + (stop - start) * i as f64 / (points - 1) as f64 }
}

/// Evaluates `points` frequencies evenly spaced from `start` to `stop` (MHz).
pub fn sweep(
    project: &Project,
    opts: &CalcOptions,
    start: f64,
    stop: f64,
    points: usize,
) -> Result<Vec<SweepPoint>, CalcError> {
    validate::sweep_points(points)?;
    let frequencies: Vec<f64> = (0..points).map(|i| sweep_frequency(start, stop, points, i)).collect();
    sweep_at(project, opts, &frequencies)
}

/// Evaluates the given frequencies (MHz); the points keep their order.
pub fn sweep_at(project: &Project, opts: &CalcOptions, frequencies: &[f64]) -> Result<Vec<SweepPoint>, CalcError> {
    // frequencies are independent: solve them in parallel
    crate::par::map(frequencies.len(), |i| {
        let f = frequencies[i];
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
