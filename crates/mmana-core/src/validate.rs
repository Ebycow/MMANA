//! Input checks shared by every calculation entry point.
//!
//! `solve` (and through it `calculate`, the sweeps, the field scans and the
//! optimiser) and `pulse_layout` all start with [`project`], so an input is
//! accepted or rejected the same way whichever entry point it arrives at.
//! MMANA's dialogs keep most values in range; project JSON, API requests,
//! `.maa` expressions (`1/0`) and the optimiser's trials do not. The ranges
//! follow the desktop's input fields; `docs/guarantees.md` lists them.

use crate::calc::{CalcOptions, CalcResult, Complex, PulseLayout};
use crate::engine::{AxisRange, FarFieldSample, NearFieldSample, NearFieldSpec, SMAX, WZMAX};
use crate::error::CalcError;
use crate::model::{Antenna, Environment, GroundKind, LoadKind, Point3, Project, Segmentation};

/// What the input is checked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Segmenting only (`pulse_layout`): sources, loads and overlapping
    /// wires do not stop it, so the layout can be shown while they are edited.
    Layout,
    /// A full calculation.
    Solve,
}

/// The smallest `SC`, as in the desktop's dialog.
const MIN_SC: f64 = 1.01;

/// Checks everything the calculation reads; returns the calculation
/// frequency (MHz).
pub(crate) fn project(project: &Project, opts: &CalcOptions, scope: Scope) -> Result<f64, CalcError> {
    let ant = &project.antenna;
    let cfq = opts.frequency_mhz.unwrap_or(ant.frequency_mhz);
    if !(cfq.is_finite() && cfq > 0.0) {
        return Err(CalcError::InvalidFrequency(cfq));
    }
    options(opts)?;
    if ant.wires.is_empty() {
        return Err(CalcError::NoWires);
    }
    wires(ant)?;
    segmentation(ant)?;
    combined_wires(ant)?;
    stack(ant)?;
    environment(&project.environment)?;
    if scope == Scope::Solve {
        sources(ant)?;
        loads(ant)?;
        overlap(ant)?;
    }
    Ok(cfq)
}

fn out_of_range(field: impl Into<String>, rule: &'static str, value: f64) -> CalcError {
    CalcError::OutOfRange { field: field.into(), rule, value }
}

fn finite(field: impl FnOnce() -> String, value: f64) -> Result<(), CalcError> {
    if value.is_finite() { Ok(()) } else { Err(out_of_range(field(), "a finite number", value)) }
}

fn options(opts: &CalcOptions) -> Result<(), CalcError> {
    if let Some(el) = opts.horizontal_cut_elevation_deg
        && !(-90.0..=90.0).contains(&el)
    {
        return Err(out_of_range("horizontal cut elevation", "from -90 to 90 degrees", el));
    }
    Ok(())
}

/// Coordinates and radii. Wires with R = 0 are not calculated, so only their
/// radius is read.
fn wires(ant: &Antenna) -> Result<(), CalcError> {
    for (i, w) in ant.wires.iter().enumerate() {
        let no = i + 1;
        finite(|| format!("wire {no} radius"), w.radius)?;
        if w.radius == 0.0 {
            continue;
        }
        for (end, p) in [("start", &w.start), ("end", &w.end)] {
            for (axis, v) in [("x", p.x), ("y", p.y), ("z", p.z)] {
                finite(|| format!("wire {no} {end} {axis}"), v)?;
            }
        }
        // finite ends 1e300 apart still overflow the length
        let d = [w.end.x - w.start.x, w.end.y - w.start.y, w.end.z - w.start.z];
        finite(|| format!("wire {no} length"), d.iter().map(|c| c * c).sum::<f64>().sqrt())?;
    }
    Ok(())
}

/// Rejects segmentation settings the tapering loops never finish with
/// (`DM2 <= 0`, or `SC` at or barely above 1: 1.000000000000001 divides by so
/// little that the loop runs on for ever).
fn segmentation(ant: &Antenna) -> Result<(), CalcError> {
    let s = &ant.auto_segmentation;
    if s.dm1 < 1 || s.dm2 < 1 {
        return Err(CalcError::InvalidAutoSegmentation("DM1 and DM2 must be at least 1"));
    }
    if !(s.sc >= MIN_SC && s.sc <= 3.0) {
        return Err(CalcError::InvalidAutoSegmentation("SC must be from 1.01 to 3"));
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

/// Section lengths and radii. A length of 0 ends the list and a radius of 0
/// repeats the previous one, as in the original.
fn combined_wires(ant: &Antenna) -> Result<(), CalcError> {
    for (i, c) in ant.combined_wires.iter().enumerate() {
        let no = i + 1;
        finite(|| format!("combined wire {no} radius key"), c.radius_key)?;
        for (j, s) in c.sections.iter().enumerate() {
            let sec = j + 1;
            if !(s.length.is_finite() && s.length >= 0.0) {
                return Err(out_of_range(format!("combined wire {no} section {sec} length"), "0 or more", s.length));
            }
            if !(s.radius.is_finite() && s.radius >= 0.0) {
                return Err(out_of_range(format!("combined wire {no} section {sec} radius"), "0 or more", s.radius));
            }
        }
    }
    Ok(())
}

/// Copy counts and spacings, then the number of segment wires the copies make
/// before they are made: each copy of a wire with R ≠ 0 expands into at least
/// one, and 100 000 copies would fill memory first. Counted as at least one
/// wire, so copies of wires that are all R = 0 are bounded too.
fn stack(ant: &Antenna) -> Result<(), CalcError> {
    let st = &ant.stack;
    // 0 copies across would drop every source of the copies made upwards
    for (field, count, spacing) in [
        ("stack horizontal", st.horizontal, st.horizontal_spacing),
        ("stack vertical", st.vertical, st.vertical_spacing),
    ] {
        if count < 1 {
            return Err(out_of_range(format!("{field} count"), "at least 1", count as f64));
        }
        if count > 1 {
            finite(|| format!("{field} spacing"), spacing)?;
            // the copies are spread over (count - 1) spacings
            finite(|| format!("{field} extent"), (count - 1) as f64 * spacing)?;
        }
    }
    if !st.is_stacked() {
        return Ok(());
    }
    let copies = i64::from(st.horizontal) * i64::from(st.vertical);
    let active = ant.wires.iter().filter(|w| w.radius != 0.0).count().max(1);
    if copies.saturating_mul(active as i64) > WZMAX as i64 {
        return Err(CalcError::TooManySegmentWires { max: WZMAX });
    }
    Ok(())
}

fn environment(env: &Environment) -> Result<(), CalcError> {
    let r = env.swr_reference.r;
    if !(r.is_finite() && r > 0.0) {
        return Err(out_of_range("SWR reference R", "a positive number", r));
    }
    finite(|| "SWR reference X".into(), env.swr_reference.x)?;
    // the whole cut; the original packs both into 16 bits
    let fb = env.fb_range;
    if !(0..=360).contains(&fb.horizontal) {
        return Err(out_of_range("F/B horizontal range", "from 0 to 360 degrees", fb.horizontal as f64));
    }
    if !(0..=180).contains(&fb.vertical) {
        return Err(out_of_range("F/B vertical range", "from 0 to 180 degrees", fb.vertical as f64));
    }
    if env.ground == GroundKind::FreeSpace {
        return Ok(());
    }
    finite(|| "height".into(), env.height)?;
    if env.ground != GroundKind::Real {
        return Ok(());
    }
    let rg = &env.real_ground;
    if rg.media.is_empty() {
        return Err(CalcError::NoGroundMedia);
    }
    for (i, m) in rg.media.iter().enumerate() {
        let no = i + 1;
        if !(m.permittivity.is_finite() && m.permittivity > 0.0) {
            return Err(out_of_range(format!("ground medium {no} permittivity"), "a positive number", m.permittivity));
        }
        if !(m.conductivity.is_finite() && m.conductivity >= 0.0) {
            return Err(out_of_range(format!("ground medium {no} conductivity"), "0 or more", m.conductivity));
        }
        if let Some(b) = m.boundary {
            finite(|| format!("ground medium {no} boundary"), b)?;
        }
        if let Some(h) = m.height {
            finite(|| format!("ground medium {no} height"), h)?;
        }
    }
    if let Some(s) = rg.screen {
        if s.radials < 0 {
            return Err(out_of_range("ground screen radials", "0 or more", s.radials as f64));
        }
        if s.radials > 0 && !(s.radial_radius.is_finite() && s.radial_radius > 0.0) {
            return Err(out_of_range("ground screen radial radius", "a positive number", s.radial_radius));
        }
    }
    Ok(())
}

fn sources(ant: &Antenna) -> Result<(), CalcError> {
    for (i, s) in ant.sources.iter().enumerate() {
        let no = i + 1;
        finite(|| format!("source {no} voltage"), s.voltage)?;
        finite(|| format!("source {no} phase"), s.phase_deg)?;
    }
    Ok(())
}

/// Load values; disabled loads are not calculated, so they are not read.
fn loads(ant: &Antenna) -> Result<(), CalcError> {
    if !ant.loads_enabled {
        return Ok(());
    }
    for (i, l) in ant.loads.iter().enumerate() {
        let no = i + 1;
        let field = || format!("load {no} value");
        match &l.kind {
            LoadKind::Lc { inductance_uh, capacitance_pf, q } => {
                for v in [*inductance_uh, *capacitance_pf, *q] {
                    finite(field, v)?;
                }
                // a capacitor of 0 pF: X = -1 / 0
                if *inductance_uh == 0.0 && *capacitance_pf == 0.0 {
                    return Err(CalcError::InvalidLoad { load: no, reason: "L and C are both 0" });
                }
            }
            LoadKind::Impedance { r, x } => {
                finite(field, *r)?;
                finite(field, *x)?;
            }
            LoadKind::SDomain { coefficients } => {
                if coefficients.len() > SMAX {
                    return Err(CalcError::InvalidLoad { load: no, reason: "an S-domain load has at most 12 terms" });
                }
                for v in coefficients.iter().flatten() {
                    finite(field, *v)?;
                }
                // Z = 0 / 0
                if coefficients.iter().all(|[_, d]| *d == 0.0) {
                    return Err(CalcError::InvalidLoad { load: no, reason: "every denominator coefficient is 0" });
                }
            }
        }
    }
    Ok(())
}

/// Rejects two wires with the same ends (in either direction), such as a
/// copy left where it was made. Their impedance matrix is singular and the
/// original carries on with NaN currents. Wires with R = 0 are not
/// calculated, so they may lie anywhere.
fn overlap(ant: &Antenna) -> Result<(), CalcError> {
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

/// Most directions (far field) or points (near field) one scan computes:
/// 1000 × 1000, about 50 MB of far-field samples. The HTTP API sets a lower
/// limit of its own; the desktop calls the core directly.
pub const MAX_FIELD_POINTS: usize = 1_000_000;

/// Most points one frequency sweep computes.
pub const MAX_SWEEP_POINTS: usize = 100_000;

/// A far-field or near-field axis: finite start and step, a count of 0 or more
/// (0, like a step of 0, means one sample).
fn axis(name: &str, a: &AxisRange) -> Result<(), CalcError> {
    finite(|| format!("{name} start"), a.start)?;
    finite(|| format!("{name} step"), a.step)?;
    if a.count < 0 {
        return Err(out_of_range(format!("{name} count"), "0 or more", a.count as f64));
    }
    // and so must the last sample (1e308 + 1e308 is not). Halved, so the
    // product does not overflow where the sum would not (1e308 - 2 × 1e308
    // is the finite -1e308); doubled back, it overflows just where the last
    // sample would
    let steps = if a.step == 0.0 { 0.0 } else { (a.count.max(1) - 1) as f64 };
    let half = 0.5 * a.start + 0.5 * a.step * steps;
    finite(|| format!("{name} end"), 2.0 * half)
}

fn field_points(axes: &[&AxisRange]) -> Result<(), CalcError> {
    if crate::contract::field_points(axes) > MAX_FIELD_POINTS {
        return Err(CalcError::TooManyFieldPoints { max: MAX_FIELD_POINTS });
    }
    Ok(())
}

pub(crate) fn far_field(zenith: &AxisRange, azimuth: &AxisRange) -> Result<(), CalcError> {
    axis("zenith", zenith)?;
    axis("azimuth", azimuth)?;
    field_points(&[zenith, azimuth])
}

pub(crate) fn near_field(spec: &NearFieldSpec) -> Result<(), CalcError> {
    axis("x", &spec.x)?;
    axis("y", &spec.y)?;
    axis("z", &spec.z)?;
    if !(spec.power_w.is_finite() && spec.power_w > 0.0) {
        return Err(out_of_range("near-field power", "a positive number", spec.power_w));
    }
    field_points(&[&spec.x, &spec.y, &spec.z])
}

/// Number of points of a sweep, or of the points asked for.
pub(crate) fn sweep_points(points: usize) -> Result<(), CalcError> {
    if !(1..=MAX_SWEEP_POINTS).contains(&points) {
        return Err(CalcError::InvalidSweepPoints { max: MAX_SWEEP_POINTS });
    }
    Ok(())
}

// Results. Finite inputs can still give values past f64, or past f32 where a
// result is stored as the original stores it: 1e100 V makes currents beyond
// f32 in the current display, 1e308 W a near field beyond f64. Every result
// is checked before it is returned, so none carries NaN or infinity (which
// JSON would turn into null).

fn representable(what: &'static str, values: impl IntoIterator<Item = f64>) -> Result<(), CalcError> {
    if values.into_iter().all(f64::is_finite) { Ok(()) } else { Err(CalcError::NotRepresentable { what }) }
}

fn complex(c: &Complex) -> [f64; 2] {
    [c.re, c.im]
}

fn point(p: &crate::model::Point3) -> [f64; 3] {
    [p.x, p.y, p.z]
}

fn floats(v: &[f32]) -> impl Iterator<Item = f64> + '_ {
    v.iter().map(|&x| f64::from(x))
}

pub(crate) fn result(r: &CalcResult) -> Result<(), CalcError> {
    let figures = [r.frequency_mhz, r.wavelength_m, r.swr, r.total_power_w]
        .into_iter()
        .chain(complex(&r.impedance))
        .chain([r.gain_dbi, r.fb_db, r.elevation_deg, r.max_horizontal_pol_dbi, r.max_vertical_pol_dbi].map(f64::from));
    representable("calculated figures", figures)?;
    let sources = r.sources.iter().flat_map(|s| {
        [complex(&s.voltage), complex(&s.current), complex(&s.impedance), [s.power_w, s.swr]].into_iter().flatten()
    });
    representable("source figures", sources)?;
    let p = &r.patterns;
    let cuts = [&p.horizontal, &p.vertical];
    let pattern = cuts.iter().flat_map(|c| {
        [c.start_deg, c.step_deg]
            .into_iter()
            .chain(floats(&c.vertical_pol))
            .chain(floats(&c.horizontal_pol))
            .chain(floats(&c.total))
    });
    representable("patterns", pattern.chain([f64::from(p.horizontal_elevation_deg)]))?;
    let currents = r.currents.iter().flat_map(|c| point(&c.position).into_iter().chain(complex(&c.current)));
    representable("currents", currents.chain(floats(&r.current_display)))?;
    let wires =
        r.segmented_wires.iter().flat_map(|w| point(&w.start).into_iter().chain(point(&w.end)).chain([w.radius]));
    representable("segmented wires", wires)
}

pub(crate) fn far_samples(samples: &[FarFieldSample]) -> Result<(), CalcError> {
    let values = samples.iter().flat_map(|s| [s.zenith, s.azimuth, s.vertical_db, s.horizontal_db, s.total_db]);
    representable("far-field values", values)
}

pub(crate) fn near_samples(samples: &[NearFieldSample]) -> Result<(), CalcError> {
    let values = samples.iter().flat_map(|s| [s.x, s.y, s.z, s.real, s.imag, s.magnitude, s.phase_deg]);
    representable("near-field values", values)
}

pub(crate) fn layout(l: &PulseLayout) -> Result<(), CalcError> {
    let pulses = l.wires.iter().flat_map(|w| w.pulses.iter().flat_map(|p| point(&p.position)));
    let placed = l.sources.iter().chain(&l.loads).flatten().flat_map(point);
    representable("pulse positions", pulses.chain(placed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::{calculate, pulse_layout, solve};
    use crate::maa;

    fn dp20() -> Project {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa/DP20.MAA");
        maa::parse_bytes(&std::fs::read(path).unwrap()).project
    }

    fn refused(what: &'static str) -> Result<(), CalcError> {
        Err(CalcError::NotRepresentable { what })
    }

    // Finite inputs have not been found to give these values; the checks
    // are there in case they do.
    #[test]
    fn a_result_with_a_value_past_f64_is_refused() {
        let o = CalcOptions::default();
        let r = calculate(&dp20(), &o).unwrap();
        assert_eq!(result(&r), Ok(()));
        let mut bad = r.clone();
        bad.impedance.im = f64::INFINITY;
        assert_eq!(result(&bad), refused("calculated figures"));
        let mut bad = r.clone();
        bad.sources[0].current.re = f64::NAN;
        assert_eq!(result(&bad), refused("source figures"));
        let mut bad = r.clone();
        bad.currents[3].position.z = f64::NAN;
        assert_eq!(result(&bad), refused("currents"));
        let mut bad = r;
        bad.segmented_wires[0].end.y = f64::NEG_INFINITY;
        assert_eq!(result(&bad), refused("segmented wires"));

        let mut samples = solve(&dp20(), &o).unwrap().far_field((0.0, 10.0, 3), (0.0, 10.0, 3)).unwrap();
        assert_eq!(far_samples(&samples), Ok(()));
        samples[4].total_db = f64::NAN;
        assert_eq!(far_samples(&samples), refused("far-field values"));

        let l = pulse_layout(&dp20(), &o).unwrap();
        assert_eq!(layout(&l), Ok(()));
        let mut bad = l.clone();
        bad.wires[0].pulses[2].position.x = f64::INFINITY;
        assert_eq!(layout(&bad), refused("pulse positions"));
        let mut bad = l;
        bad.sources[0].as_mut().unwrap().y = f64::NAN;
        assert_eq!(layout(&bad), refused("pulse positions"));
    }
}
