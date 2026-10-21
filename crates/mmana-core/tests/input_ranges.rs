//! Every input range in docs/guarantees.md: a value on the boundary is
//! accepted, one just outside is rejected, and `calculate` and `pulse_layout`
//! agree except where the layout is documented to be lenient (sources, loads,
//! overlapping wires).

use mmana_core::contract::{self, SweepRequest};
use mmana_core::{
    AxisRange, CalcError, CalcOptions, CombinedSection, CombinedWire, GroundKind, GroundScreen, Load, LoadKind,
    MAX_FIELD_POINTS, MAX_SWEEP_POINTS, Medium, NearFieldKind, NearFieldSpec, Project, calculate, maa, pulse_layout,
    solve,
};
use std::path::Path;

fn dp20() -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa/DP20.MAA");
    maa::parse_bytes(&std::fs::read(path).unwrap()).project
}

fn out_of_range(field: &str, rule: &'static str, value: f64) -> Result<(), CalcError> {
    Err(CalcError::OutOfRange { field: field.into(), rule, value })
}

/// Compares errors with NaN values as equal.
fn same(a: &Result<(), CalcError>, b: &Result<(), CalcError>) -> bool {
    match (a, b) {
        (
            Err(CalcError::OutOfRange { field: f1, rule: r1, value: v1 }),
            Err(CalcError::OutOfRange { field: f2, rule: r2, value: v2 }),
        ) => f1 == f2 && r1 == r2 && (v1 == v2 || (v1.is_nan() && v2.is_nan())),
        (Err(CalcError::InvalidFrequency(a)), Err(CalcError::InvalidFrequency(b))) => {
            a == b || (a.is_nan() && b.is_nan())
        }
        _ => a == b,
    }
}

struct Case {
    name: &'static str,
    edit: fn(&mut Project, &mut CalcOptions),
    solve: Result<(), CalcError>,
    /// `None`: as `solve`.
    layout: Option<Result<(), CalcError>>,
}

fn case(name: &'static str, edit: fn(&mut Project, &mut CalcOptions), solve: Result<(), CalcError>) -> Case {
    Case { name, edit, solve, layout: None }
}

/// Checked by the calculation only; the layout is still produced.
fn solve_only(name: &'static str, edit: fn(&mut Project, &mut CalcOptions), solve: Result<(), CalcError>) -> Case {
    Case { name, edit, solve, layout: Some(Ok(())) }
}

fn run(cases: Vec<Case>) {
    let mut failures = Vec::new();
    for c in cases {
        let mut p = dp20();
        let mut o = CalcOptions::default();
        (c.edit)(&mut p, &mut o);
        let solved = calculate(&p, &o).map(|_| ());
        let laid = pulse_layout(&p, &o).map(|_| ());
        let layout = c.layout.unwrap_or_else(|| c.solve.clone());
        if !same(&solved, &c.solve) {
            failures.push(format!("{}: calculate gave {solved:?}, expected {:?}", c.name, c.solve));
        }
        if !same(&laid, &layout) {
            failures.push(format!("{}: pulse_layout gave {laid:?}, expected {layout:?}", c.name));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

const NAN: f64 = f64::NAN;
const INF: f64 = f64::INFINITY;

#[test]
fn frequency_and_options() {
    run(vec![
        case("design frequency 0.001", |p, _| p.antenna.frequency_mhz = 0.001, Ok(())),
        case("design frequency 0", |p, _| p.antenna.frequency_mhz = 0.0, Err(CalcError::InvalidFrequency(0.0))),
        case("design frequency NaN", |p, _| p.antenna.frequency_mhz = NAN, Err(CalcError::InvalidFrequency(NAN))),
        case("frequency option -1", |_, o| o.frequency_mhz = Some(-1.0), Err(CalcError::InvalidFrequency(-1.0))),
        case("frequency option inf", |_, o| o.frequency_mhz = Some(INF), Err(CalcError::InvalidFrequency(INF))),
        case("cut elevation 90", |_, o| o.horizontal_cut_elevation_deg = Some(90.0), Ok(())),
        case("cut elevation -90", |_, o| o.horizontal_cut_elevation_deg = Some(-90.0), Ok(())),
        case(
            "cut elevation 90.5",
            |_, o| o.horizontal_cut_elevation_deg = Some(90.5),
            out_of_range("horizontal cut elevation", "from -90 to 90 degrees", 90.5),
        ),
        case(
            "cut elevation NaN",
            |_, o| o.horizontal_cut_elevation_deg = Some(NAN),
            out_of_range("horizontal cut elevation", "from -90 to 90 degrees", NAN),
        ),
        case("max pulses 0", |_, o| o.max_pulses = 0, Err(CalcError::TooManyPulses { max: 0 })),
        case("max pulses 17", |_, o| o.max_pulses = 17, Ok(())),
        case("max pulses 16", |_, o| o.max_pulses = 16, Err(CalcError::TooManyPulses { max: 16 })),
    ]);
}

#[test]
fn wires() {
    run(vec![
        case(
            "radius NaN",
            |p, _| p.antenna.wires[0].radius = NAN,
            out_of_range("wire 1 radius", "a finite number", NAN),
        ),
        case(
            "radius inf",
            |p, _| p.antenna.wires[0].radius = INF,
            out_of_range("wire 1 radius", "a finite number", INF),
        ),
        case(
            "start x NaN",
            |p, _| p.antenna.wires[0].start.x = NAN,
            out_of_range("wire 1 start x", "a finite number", NAN),
        ),
        case("end z inf", |p, _| p.antenna.wires[0].end.z = INF, out_of_range("wire 1 end z", "a finite number", INF)),
        case(
            "ends 2e300 apart",
            |p, _| {
                p.antenna.wires[0].start.x = -1e300;
                p.antenna.wires[0].end.x = 1e300;
            },
            out_of_range("wire 1 length", "a finite number", INF),
        ),
        case(
            "a disabled wire may have any ends",
            |p, _| {
                let mut w = p.antenna.wires[0].clone();
                w.radius = 0.0;
                w.start.x = NAN;
                p.antenna.wires.push(w);
            },
            Ok(()),
        ),
        case(
            "no wires",
            |p, _| {
                p.antenna.wires.clear();
            },
            Err(CalcError::NoWires),
        ),
        solve_only(
            "two wires on top of each other",
            |p, _| {
                let w = p.antenna.wires[0].clone();
                p.antenna.wires.push(w);
            },
            Err(CalcError::OverlappingWires { first: 1, second: 2 }),
        ),
    ]);
}

#[test]
fn stacking() {
    run(vec![
        case("1 × 1", |p, _| p.antenna.stack.horizontal = 1, Ok(())),
        case(
            "0 across",
            |p, _| {
                p.antenna.stack.horizontal = 0;
                p.antenna.stack.vertical = 2;
            },
            out_of_range("stack horizontal count", "at least 1", 0.0),
        ),
        case("-3 up", |p, _| p.antenna.stack.vertical = -3, out_of_range("stack vertical count", "at least 1", -3.0)),
        case(
            "spacing NaN with 2 across",
            |p, _| {
                p.antenna.stack.horizontal = 2;
                p.antenna.stack.horizontal_spacing = NAN;
            },
            out_of_range("stack horizontal spacing", "a finite number", NAN),
        ),
        case(
            "spacing inf with 2 up",
            |p, _| {
                p.antenna.stack.vertical = 2;
                p.antenna.stack.vertical_spacing = INF;
            },
            out_of_range("stack vertical spacing", "a finite number", INF),
        ),
        case("an unused spacing is not read", |p, _| p.antenna.stack.horizontal_spacing = NAN, Ok(())),
        case(
            "4096 copies of one wire",
            |p, _| p.antenna.stack.horizontal = 4096,
            Err(CalcError::TooManyPulses { max: 4096 }),
        ),
        case(
            "4097 copies of one wire",
            |p, _| p.antenna.stack.horizontal = 4097,
            Err(CalcError::TooManySegmentWires { max: 4096 }),
        ),
    ]);
}

#[test]
fn environment() {
    fn real(p: &mut Project) {
        p.environment.ground = GroundKind::Real;
    }
    run(vec![
        case("SWR R 0.001", |p, _| p.environment.swr_reference.r = 0.001, Ok(())),
        case(
            "SWR R 0",
            |p, _| p.environment.swr_reference.r = 0.0,
            out_of_range("SWR reference R", "a positive number", 0.0),
        ),
        case(
            "SWR R NaN",
            |p, _| p.environment.swr_reference.r = NAN,
            out_of_range("SWR reference R", "a positive number", NAN),
        ),
        case(
            "SWR X inf",
            |p, _| p.environment.swr_reference.x = INF,
            out_of_range("SWR reference X", "a finite number", INF),
        ),
        case("F/B horizontal 360", |p, _| p.environment.fb_range.horizontal = 360, Ok(())),
        case(
            "F/B horizontal 361",
            |p, _| p.environment.fb_range.horizontal = 361,
            out_of_range("F/B horizontal range", "from 0 to 360 degrees", 361.0),
        ),
        case(
            "F/B horizontal -1",
            |p, _| p.environment.fb_range.horizontal = -1,
            out_of_range("F/B horizontal range", "from 0 to 360 degrees", -1.0),
        ),
        case("F/B vertical 0", |p, _| p.environment.fb_range.vertical = 0, Ok(())),
        case("F/B vertical 180", |p, _| p.environment.fb_range.vertical = 180, Ok(())),
        case(
            "F/B vertical 181",
            |p, _| p.environment.fb_range.vertical = 181,
            out_of_range("F/B vertical range", "from 0 to 180 degrees", 181.0),
        ),
        case(
            "height is not read in free space",
            |p, _| {
                p.environment.ground = GroundKind::FreeSpace;
                p.environment.height = NAN;
            },
            Ok(()),
        ),
        case(
            "height NaN over perfect ground",
            |p, _| {
                p.environment.ground = GroundKind::Perfect;
                p.environment.height = NAN;
            },
            out_of_range("height", "a finite number", NAN),
        ),
        case(
            "no media",
            |p, _| {
                real(p);
                p.environment.real_ground.media.clear();
            },
            Err(CalcError::NoGroundMedia),
        ),
        case(
            "media are not read over perfect ground",
            |p, _| {
                p.environment.ground = GroundKind::Perfect;
                p.environment.real_ground.media.clear();
            },
            Ok(()),
        ),
        case(
            "permittivity 1/3",
            |p, _| {
                real(p);
                p.environment.real_ground.media[0].permittivity = 1.0 / 3.0;
            },
            Ok(()),
        ),
        case(
            "permittivity 0",
            |p, _| {
                real(p);
                p.environment.real_ground.media[0].permittivity = 0.0;
            },
            out_of_range("ground medium 1 permittivity", "a positive number", 0.0),
        ),
        case(
            "conductivity 0",
            |p, _| {
                real(p);
                p.environment.real_ground.media[0].conductivity = 0.0;
            },
            Ok(()),
        ),
        case(
            "conductivity -1e-9",
            |p, _| {
                real(p);
                p.environment.real_ground.media[0].conductivity = -1e-9;
            },
            out_of_range("ground medium 1 conductivity", "0 or more", -1e-9),
        ),
        case(
            "second medium boundary NaN",
            |p, _| {
                real(p);
                p.environment.real_ground.media =
                    vec![Medium { boundary: Some(NAN), ..Medium::default() }, Medium::default()];
            },
            out_of_range("ground medium 1 boundary", "a finite number", NAN),
        ),
        case(
            "medium height inf",
            |p, _| {
                real(p);
                p.environment.real_ground.media[0].height = Some(INF);
            },
            out_of_range("ground medium 1 height", "a finite number", INF),
        ),
        case(
            "no radials, radius 0",
            |p, _| {
                real(p);
                p.environment.real_ground.screen = Some(GroundScreen { radials: 0, radial_radius: 0.0 });
            },
            Ok(()),
        ),
        case(
            "-1 radials",
            |p, _| {
                real(p);
                p.environment.real_ground.screen = Some(GroundScreen { radials: -1, radial_radius: 1.0 });
            },
            out_of_range("ground screen radials", "0 or more", -1.0),
        ),
        case(
            "16 radials of radius 0",
            |p, _| {
                real(p);
                p.environment.real_ground.screen = Some(GroundScreen { radials: 16, radial_radius: 0.0 });
            },
            out_of_range("ground screen radial radius", "a positive number", 0.0),
        ),
    ]);
}

fn combined(p: &mut Project, length: f64, radius: f64) {
    p.antenna.wires[0].radius = -1.0;
    p.antenna.combined_wires = vec![CombinedWire {
        radius_key: -1.0,
        mode: 0,
        sections: vec![CombinedSection { length: 1.0, radius: 0.01 }, CombinedSection { length, radius }],
    }];
}

#[test]
fn combined_wires() {
    run(vec![
        case("section length 0 ends the list", |p, _| combined(p, 0.0, 0.02), Ok(())),
        case("section radius 0 repeats the last", |p, _| combined(p, 1.0, 0.0), Ok(())),
        case(
            "section length -1",
            |p, _| combined(p, -1.0, 0.02),
            out_of_range("combined wire 1 section 2 length", "0 or more", -1.0),
        ),
        case(
            "section length NaN",
            |p, _| combined(p, NAN, 0.02),
            out_of_range("combined wire 1 section 2 length", "0 or more", NAN),
        ),
        case(
            "section radius -0.01",
            |p, _| combined(p, 1.0, -0.01),
            out_of_range("combined wire 1 section 2 radius", "0 or more", -0.01),
        ),
    ]);
}

fn with_load(p: &mut Project, kind: LoadKind) {
    p.antenna.loads_enabled = true;
    p.antenna.loads = vec![Load { pulse: "w1b".into(), kind }];
}

#[test]
fn sources_and_loads() {
    run(vec![
        solve_only(
            "voltage NaN",
            |p, _| p.antenna.sources[0].voltage = NAN,
            out_of_range("source 1 voltage", "a finite number", NAN),
        ),
        solve_only(
            "phase inf",
            |p, _| p.antenna.sources[0].phase_deg = INF,
            out_of_range("source 1 phase", "a finite number", INF),
        ),
        solve_only("voltage 0", |p, _| p.antenna.sources[0].voltage = 0.0, Err(CalcError::NoExcitation)),
        case("negative voltage", |p, _| p.antenna.sources[0].voltage = -1.0, Ok(())),
        // 2^32 + 1 wrapped to pulse 1
        solve_only(
            "pulse number past i32",
            |p, _| p.antenna.sources[0].pulse = "4294967297".into(),
            Err(CalcError::PulseOutOfRange { what: "source", name: "4294967297".into(), max: 17 }),
        ),
        solve_only(
            "wire number past i32",
            |p, _| p.antenna.sources[0].pulse = "w4294967297c".into(),
            Err(CalcError::PulseOutOfRange { what: "source", name: "w4294967297c".into(), max: 17 }),
        ),
        solve_only(
            "L and C both 0",
            |p, _| with_load(p, LoadKind::Lc { inductance_uh: 0.0, capacitance_pf: 0.0, q: 0.0 }),
            Err(CalcError::InvalidLoad { load: 1, reason: "L and C are both 0" }),
        ),
        case(
            "a disabled load is not read",
            |p, _| {
                with_load(p, LoadKind::Lc { inductance_uh: 0.0, capacitance_pf: 0.0, q: 0.0 });
                p.antenna.loads_enabled = false;
            },
            Ok(()),
        ),
        solve_only(
            "Q NaN",
            |p, _| with_load(p, LoadKind::Lc { inductance_uh: 1.0, capacitance_pf: 0.0, q: NAN }),
            out_of_range("load 1 value", "a finite number", NAN),
        ),
        solve_only(
            "R inf",
            |p, _| with_load(p, LoadKind::Impedance { r: INF, x: 0.0 }),
            out_of_range("load 1 value", "a finite number", INF),
        ),
        case("R -1e6", |p, _| with_load(p, LoadKind::Impedance { r: -1e6, x: 0.0 }), Ok(())),
        solve_only(
            "S-domain without terms",
            |p, _| with_load(p, LoadKind::SDomain { coefficients: vec![] }),
            Err(CalcError::InvalidLoad { load: 1, reason: "every denominator coefficient is 0" }),
        ),
        solve_only(
            "S-domain 1 / 0",
            |p, _| with_load(p, LoadKind::SDomain { coefficients: vec![[1.0, 0.0], [0.0, 0.0]] }),
            Err(CalcError::InvalidLoad { load: 1, reason: "every denominator coefficient is 0" }),
        ),
        case(
            "S-domain 12 terms",
            |p, _| with_load(p, LoadKind::SDomain { coefficients: vec![[1.0, 1.0]; 12] }),
            Ok(()),
        ),
        // the 13th was dropped without a word
        solve_only(
            "S-domain 13 terms",
            |p, _| with_load(p, LoadKind::SDomain { coefficients: vec![[1.0, 1.0]; 13] }),
            Err(CalcError::InvalidLoad { load: 1, reason: "an S-domain load has at most 12 terms" }),
        ),
    ]);
}

fn axis(start: f64, step: f64, count: i32) -> AxisRange {
    AxisRange { start, step, count }
}

#[test]
fn far_field_scans() {
    let solved = solve(&dp20(), &CalcOptions::default()).unwrap();
    let scan = |z: AxisRange, a: AxisRange| solved.far_field((z.start, z.step, z.count), (a.start, a.step, a.count));
    assert_eq!(scan(axis(0.0, 1.0, 0), axis(0.0, 1.0, 3)).unwrap().len(), 3);
    assert_eq!(scan(axis(0.0, 0.0, 50), axis(0.0, 1.0, 3)).unwrap().len(), 3);
    assert_eq!(
        scan(axis(0.0, 1.0, -1), axis(0.0, 1.0, 3)).map(|_| ()),
        out_of_range("zenith count", "0 or more", -1.0)
    );
    assert!(matches!(
        scan(axis(0.0, NAN, 3), axis(0.0, 1.0, 3)),
        Err(CalcError::OutOfRange { field, .. }) if field == "zenith step"
    ));
    assert_eq!(
        scan(axis(0.0, 1.0, 3), axis(INF, 1.0, 3)).map(|_| ()),
        out_of_range("azimuth start", "a finite number", INF)
    );
    // i32 products used to wrap: 100 000² is 1 410 065 408 in 32 bits
    let max = Err(CalcError::TooManyFieldPoints { max: MAX_FIELD_POINTS });
    assert_eq!(scan(axis(0.0, 1.0, 1000), axis(0.0, 1.0, 1001)).map(|_| ()), max);
    assert_eq!(scan(axis(0.0, 1.0, 100_000), axis(0.0, 1.0, 100_000)).map(|_| ()), max);
}

#[test]
fn near_field_scans() {
    let solved = solve(&dp20(), &CalcOptions::default()).unwrap();
    let base = NearFieldSpec {
        kind: NearFieldKind::Electric,
        power_w: 100.0,
        x: axis(0.0, 1.0, 2),
        y: axis(0.0, 0.0, 1),
        z: axis(1.0, 0.0, 1),
    };
    assert_eq!(solved.near_field(&base).unwrap().len(), 2 * 3);
    let check = |spec: NearFieldSpec| solved.near_field(&spec).map(|_| ());
    assert_eq!(
        check(NearFieldSpec { power_w: 0.0, ..base }),
        out_of_range("near-field power", "a positive number", 0.0)
    );
    assert!(matches!(
        check(NearFieldSpec { power_w: NAN, ..base }),
        Err(CalcError::OutOfRange { field, .. }) if field == "near-field power"
    ));
    assert_eq!(check(NearFieldSpec { y: axis(0.0, 1.0, -2), ..base }), out_of_range("y count", "0 or more", -2.0));
    // 2^21 · 2^21 · 2^22 is 2^64, which wrapped to 0 in usize
    assert_eq!(
        check(NearFieldSpec {
            x: axis(0.0, 1.0, 1 << 21),
            y: axis(0.0, 1.0, 1 << 21),
            z: axis(0.0, 1.0, 1 << 22),
            ..base
        }),
        Err(CalcError::TooManyFieldPoints { max: MAX_FIELD_POINTS })
    );
}

#[test]
fn sweep_points() {
    let p = dp20();
    let o = CalcOptions::default();
    let invalid = Err(CalcError::InvalidSweepPoints { max: MAX_SWEEP_POINTS });
    assert_eq!(mmana_core::sweep(&p, &o, 14.0, 14.1, 1).unwrap().len(), 1);
    assert_eq!(mmana_core::sweep(&p, &o, 14.0, 14.1, 0).map(|_| ()), invalid);
    assert_eq!(mmana_core::sweep(&p, &o, 14.0, 14.1, MAX_SWEEP_POINTS + 1).map(|_| ()), invalid);
    let req = |points, indices| SweepRequest {
        project: p.clone(),
        options: o.clone(),
        start_mhz: 14.0,
        stop_mhz: 14.1,
        points,
        indices: Some(indices),
    };
    assert_eq!(contract::sweep(&req(3, vec![2, 0])).unwrap().points.len(), 2);
    assert_eq!(contract::sweep(&req(0, vec![0])).map(|_| ()), invalid);
    assert_eq!(
        contract::sweep(&req(3, vec![3])).map(|_| ()),
        Err(CalcError::SweepPointOutOfRange { index: 3, points: 3 })
    );
    assert_eq!(contract::sweep(&req(3, vec![0; MAX_SWEEP_POINTS + 1])).map(|_| ()), invalid);
}
