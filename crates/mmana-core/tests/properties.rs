//! Properties a correct calculation has, checked without the original:
//! how the results must change (or stay) when the input is changed in a
//! known way, and physical laws such as power conservation and image
//! theory. The golden tests show the port does what the original does;
//! these show what it does is right, including where the original is not.
//!
//! Changes that only scale by a power of two, rename or reorder are exact in
//! floating point, so they are compared bit for bit. The others are compared
//! within tolerances stated with each test.

use mmana_core::{
    CalcOptions, CalcResult, Complex, GroundKind, Load, LoadKind, Point3, Project, Segmentation, Source, WarningCode,
    Wire, calculate, maa, pulse_layout, solve, sweep_at,
};
use std::f64::consts::PI;
use std::path::Path;

fn sample(name: &str) -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa").join(name);
    maa::parse_bytes(&std::fs::read(path).unwrap()).project
}

fn wire(start: (f64, f64, f64), end: (f64, f64, f64), segments: i32) -> Wire {
    Wire {
        start: Point3::new(start.0, start.1, start.2),
        end: Point3::new(end.0, end.1, end.2),
        radius: 0.001,
        segmentation: Segmentation::Fixed { count: segments },
        note: String::new(),
        group: String::new(),
    }
}

fn source(pulse: &str, voltage: f64, phase_deg: f64) -> Source {
    Source { pulse: pulse.into(), phase_deg, voltage }
}

fn project(wires: Vec<Wire>, sources: Vec<Source>) -> Project {
    let mut p = Project::default();
    p.antenna.wires = wires;
    p.antenna.sources = sources;
    p
}

/// A half-wave dipole along Y at 14.05 MHz in free space, 20 segments
/// (19 pulses, the centre one on the middle of the wire).
fn dipole() -> Project {
    project(vec![wire((0.0, -5.2, 0.0), (0.0, 5.2, 0.0), 20)], vec![source("w1c", 1.0, 0.0)])
}

/// A 3-element Yagi along X in free space, fed on the middle element.
fn yagi() -> Project {
    project(
        vec![
            wire((-2.0, -5.5, 0.0), (-2.0, 5.5, 0.0), 22),
            wire((0.0, -5.2, 0.0), (0.0, 5.2, 0.0), 22),
            wire((2.0, -4.9, 0.0), (2.0, 4.9, 0.0), 22),
        ],
        vec![source("w2c", 1.0, 0.0)],
    )
}

fn calc(p: &Project) -> CalcResult {
    calculate(p, &CalcOptions::default()).unwrap()
}

fn abs(c: Complex) -> f64 {
    c.re.hypot(c.im)
}

fn sub(a: Complex, b: Complex) -> Complex {
    Complex { re: a.re - b.re, im: a.im - b.im }
}

/// |a − b| relative to the larger of |a| and |b|.
fn rel(a: Complex, b: Complex) -> f64 {
    abs(sub(a, b)) / abs(a).max(abs(b)).max(f64::MIN_POSITIVE)
}

/// Bit-for-bit equality, reporting the sources and currents first so a
/// failure does not print every pattern sample.
#[track_caller]
fn assert_same(a: &CalcResult, b: &CalcResult, what: &str) {
    assert_eq!(a.sources, b.sources, "{what}: sources");
    assert_eq!(a.currents, b.currents, "{what}: currents");
    assert!(a == b, "{what}: the results differ beyond the sources and currents");
}

fn max_db_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
}

/// The samples this test suite checks most relations on: free space, perfect
/// and real ground, loads, stacking, tapering and combined wires.
const SAMPLES: [&str; 6] = ["DP20.MAA", "3EL20.MAA", "GP40.MAA", "VERT20.MAA", "HB9CV.MAA", "4X12.MAA"];

#[test]
fn doubling_every_voltage_doubles_the_currents_and_changes_nothing_else() {
    // a factor of 2 is exact in binary floating point, so the solution is too
    for name in SAMPLES {
        let p = sample(name);
        let mut q = p.clone();
        q.antenna.sources.iter_mut().for_each(|s| s.voltage *= 2.0);
        let (a, b) = (calc(&p), calc(&q));
        for (s, t) in a.sources.iter().zip(&b.sources) {
            assert_eq!((t.current.re, t.current.im), (2.0 * s.current.re, 2.0 * s.current.im), "{name}");
            assert_eq!(t.impedance, s.impedance, "{name}");
            assert_eq!(t.swr, s.swr, "{name}");
            assert_eq!(t.power_w, 4.0 * s.power_w, "{name}");
        }
        assert_eq!(b.total_power_w, 4.0 * a.total_power_w, "{name}");
        for (c, d) in a.currents.iter().zip(&b.currents) {
            assert_eq!((d.current.re, d.current.im), (2.0 * c.current.re, 2.0 * c.current.im), "{name}");
        }
        assert_eq!(b.patterns, a.patterns, "{name}");
        assert_eq!((b.gain_dbi, b.fb_db, b.elevation_deg), (a.gain_dbi, a.fb_db, a.elevation_deg), "{name}");
    }
}

#[test]
fn scaling_every_voltage_by_3_scales_the_currents_within_rounding() {
    // the matrix is stored in f32, so 3× is not exact: 1e-6 is a few f32 ulps
    for name in SAMPLES {
        let p = sample(name);
        let mut q = p.clone();
        q.antenna.sources.iter_mut().for_each(|s| s.voltage *= 3.0);
        let (a, b) = (calc(&p), calc(&q));
        for (s, t) in a.sources.iter().zip(&b.sources) {
            let scaled = Complex { re: 3.0 * s.current.re, im: 3.0 * s.current.im };
            assert!(rel(t.current, scaled) < 1e-6, "{name}: {:?} vs {scaled:?}", t.current);
            assert!(rel(t.impedance, s.impedance) < 1e-6, "{name}");
        }
        assert!((b.total_power_w / a.total_power_w - 9.0).abs() < 1e-5, "{name}");
        assert!(max_db_diff(&a.patterns.horizontal.total, &b.patterns.horizontal.total) < 1e-4, "{name}");
        assert!((a.gain_dbi - b.gain_dbi).abs() < 1e-4, "{name}");
    }
}

#[test]
fn shifting_every_phase_rotates_the_currents_and_keeps_the_impedance() {
    for name in SAMPLES {
        let p = sample(name);
        let mut q = p.clone();
        q.antenna.sources.iter_mut().for_each(|s| s.phase_deg += 30.0);
        let (a, b) = (calc(&p), calc(&q));
        let (c, s) = ((30.0 * PI / 180.0).cos(), (30.0 * PI / 180.0).sin());
        for (x, y) in a.sources.iter().zip(&b.sources) {
            let rotated = Complex { re: x.current.re * c - x.current.im * s, im: x.current.re * s + x.current.im * c };
            assert!(rel(y.current, rotated) < 1e-6, "{name}");
            assert!(rel(y.impedance, x.impedance) < 1e-6, "{name}");
            assert!((y.power_w - x.power_w).abs() <= 1e-6 * x.power_w.abs(), "{name}");
        }
        assert!((a.gain_dbi - b.gain_dbi).abs() < 1e-4, "{name}");
    }
}

#[test]
fn total_power_is_the_sum_of_the_source_powers() {
    for name in SAMPLES {
        let r = calc(&sample(name));
        let sum: f64 = r.sources.iter().map(|s| s.power_w).sum();
        assert!((r.total_power_w - sum).abs() <= 1e-12 * sum.abs(), "{name}: {} vs {sum}", r.total_power_w);
        // P = Re(V · conj(I)) / 2 at each source
        for s in &r.sources {
            let p = (s.voltage.re * s.current.re + s.voltage.im * s.current.im) / 2.0;
            assert!((s.power_w - p).abs() <= 1e-12 * p.abs().max(1e-30), "{name}");
        }
    }
}

#[test]
fn swr_follows_from_the_impedance_and_the_reference() {
    for name in SAMPLES {
        let mut p = sample(name);
        p.environment.swr_reference.x = 0.0;
        let r0 = p.environment.swr_reference.r;
        for s in &calc(&p).sources {
            let z = s.impedance;
            let g = abs(Complex { re: z.re - r0, im: z.im }) / abs(Complex { re: z.re + r0, im: z.im });
            let swr = (1.0 + g) / (1.0 - g);
            // stored as f32
            assert!((s.swr - swr).abs() <= 1e-6 * swr, "{name}: {} vs {swr}", s.swr);
        }
    }
}

/// Each source moved from its name to the pulse number the name resolves to.
fn numbered(p: &Project) -> Project {
    let layout = pulse_layout(p, &CalcOptions::default()).unwrap();
    let number = |name: &str| {
        layout
            .wires
            .iter()
            .flat_map(|w| &w.pulses)
            .find(|q| q.name.eq_ignore_ascii_case(name))
            .map(|q| q.pulse.to_string())
    };
    let mut q = p.clone();
    for s in &mut q.antenna.sources {
        if let Some(n) = number(&s.pulse) {
            s.pulse = n;
        }
    }
    for l in &mut q.antenna.loads {
        if let Some(n) = number(&l.pulse) {
            l.pulse = n;
        }
    }
    q
}

#[test]
fn a_pulse_named_or_numbered_gives_the_same_result() {
    for name in SAMPLES {
        let p = sample(name);
        let q = numbered(&p);
        assert_ne!(p.antenna.sources, q.antenna.sources, "{name}: nothing was renumbered");
        assert_same(&calc(&p), &calc(&q), name);
    }
}

#[test]
fn a_pulse_named_or_numbered_gives_the_same_result_when_stacked() {
    // the copies' numbered sources once landed on the first copy
    for (h, v) in [(2, 1), (1, 3), (2, 2)] {
        let mut p = yagi();
        p.antenna.loads_enabled = true;
        p.antenna.loads = vec![Load { pulse: "w1b5".into(), kind: LoadKind::Impedance { r: 10.0, x: 5.0 } }];
        p.antenna.stack.horizontal = h;
        p.antenna.stack.vertical = v;
        p.antenna.stack.horizontal_spacing = 12.0;
        p.antenna.stack.vertical_spacing = 10.0;
        let q = numbered(&p);
        assert_same(&calc(&p), &calc(&q), &format!("{h} × {v}"));
    }
}

#[test]
fn every_name_of_a_pulse_resolves_to_that_pulse() {
    // feeding a pulse by any of its listed names is feeding that pulse
    let p = yagi();
    let layout = pulse_layout(&p, &CalcOptions::default()).unwrap();
    for w in &layout.wires {
        for q in w.pulses.iter().step_by(4) {
            let mut by_name = p.clone();
            by_name.antenna.sources[0].pulse = q.name.clone();
            let mut by_number = p.clone();
            by_number.antenna.sources[0].pulse = q.pulse.to_string();
            let r = calc(&by_name);
            assert_eq!(r.sources[0].pulse, q.pulse, "{}", q.name);
            assert_same(&r, &calc(&by_number), &q.name);
        }
    }
}

#[test]
fn a_stack_is_its_copies_side_by_side() {
    // TMainWnd::Stack builds the copies and splits the voltage between them;
    // doing so by hand must give the same result
    let p = yagi();
    let d = 12.0;
    let mut stacked = p.clone();
    stacked.antenna.stack.horizontal = 2;
    stacked.antenna.stack.horizontal_spacing = d;
    let mut by_hand = p.clone();
    by_hand.antenna.wires.clear();
    let mut yc = -d / 2.0;
    for _ in 0..2 {
        for w in &p.antenna.wires {
            let mut w = w.clone();
            w.start.y += yc;
            w.end.y += yc;
            by_hand.antenna.wires.push(w);
        }
        yc += d;
    }
    by_hand.antenna.sources = vec![source("w2c", 0.5, 0.0), source("w5c", 0.5, 0.0)];
    assert_same(&calc(&stacked), &calc(&by_hand), "stacked and by hand");
}

#[test]
fn disabled_loads_calculate_as_no_loads() {
    for name in SAMPLES {
        let mut p = sample(name);
        if p.antenna.loads.is_empty() {
            p.antenna.loads = vec![Load { pulse: "w1b".into(), kind: LoadKind::Impedance { r: 50.0, x: 20.0 } }];
        }
        p.antenna.loads_enabled = false;
        let mut q = p.clone();
        q.antenna.loads.clear();
        assert_same(&calc(&p), &calc(&q), name);
    }
}

#[test]
fn a_load_of_0_ohms_is_no_load() {
    // adding 0 to the matrix diagonal is exact
    for name in ["DP20.MAA", "3EL20.MAA", "GP40.MAA"] {
        let p = sample(name);
        let mut q = p.clone();
        q.antenna.loads_enabled = true;
        q.antenna.loads.push(Load { pulse: "w1b2".into(), kind: LoadKind::Impedance { r: 0.0, x: 0.0 } });
        let mut r = p.clone();
        r.antenna.loads_enabled = true;
        assert_same(&calc(&q), &calc(&r), name);
    }
}

#[test]
fn two_loads_on_one_pulse_are_in_series() {
    // loads add to the diagonal one after the other (rounded to f32 in
    // between), so the sum matches to f32 rounding
    let with = |loads: Vec<(f64, f64)>| {
        let mut p = dipole();
        p.antenna.loads_enabled = true;
        p.antenna.loads =
            loads.into_iter().map(|(r, x)| Load { pulse: "7".into(), kind: LoadKind::Impedance { r, x } }).collect();
        calc(&p)
    };
    let (two, one) = (with(vec![(12.0, 30.0), (8.0, -55.0)]), with(vec![(20.0, -25.0)]));
    assert!(rel(two.impedance, one.impedance) < 1e-5, "{:?} vs {:?}", two.impedance, one.impedance);
}

#[test]
fn a_sweep_point_is_the_single_calculation_at_its_frequency() {
    for name in ["DP20.MAA", "3EL20.MAA", "4X12.MAA"] {
        let p = sample(name);
        let f0 = p.antenna.frequency_mhz;
        let frequencies = [f0 * 0.97, f0, f0 * 1.031];
        let points = sweep_at(&p, &CalcOptions::default(), &frequencies).unwrap();
        for (pt, &f) in points.iter().zip(&frequencies) {
            let r = calculate(&p, &CalcOptions { frequency_mhz: Some(f), ..CalcOptions::default() }).unwrap();
            assert_eq!(pt.frequency_mhz, f);
            assert_eq!((pt.impedance, pt.swr), (r.impedance, r.swr), "{name} at {f}");
            assert_eq!((pt.gain_dbi, pt.fb_db, pt.elevation_deg), (r.gain_dbi, r.fb_db, r.elevation_deg), "{name}");
        }
    }
}

#[test]
fn a_calculation_does_not_depend_on_the_ones_before_it_or_beside_it() {
    let projects: Vec<Project> = SAMPLES.iter().map(|n| sample(n)).collect();
    let first: Vec<CalcResult> = projects.iter().map(calc).collect();
    // in reverse order, and all at once on other threads
    for ((p, r), name) in projects.iter().zip(&first).zip(SAMPLES).rev() {
        assert_same(&calc(p), r, name);
    }
    std::thread::scope(|s| {
        let handles: Vec<_> = projects.iter().map(|p| s.spawn(move || calc(p))).collect();
        for ((h, r), name) in handles.into_iter().zip(&first).zip(SAMPLES) {
            assert_same(&h.join().unwrap(), r, name);
        }
    });
}

/// Gain integrated over the whole sphere, from a 2° midpoint grid: P_radiated
/// / P_in. Below a ground's horizon there is no field, so over ground only
/// the upper half counts (the original gives the image's field there).
fn radiated_fraction(p: &Project) -> f64 {
    let solved = solve(p, &CalcOptions::default()).unwrap();
    let step = 2.0f64;
    let samples = solved.far_field((step / 2.0, step, 90), (step / 2.0, step, 180)).unwrap();
    if p.environment.ground != GroundKind::FreeSpace {
        let below = samples.iter().filter(|s| s.zenith > 90.0);
        assert!(below.clone().all(|s| s.total_db == -999.0), "a field below the ground");
    }
    let cell = (step * PI / 180.0).powi(2);
    samples.iter().map(|s| 10f64.powf(s.total_db / 10.0) * (s.zenith * PI / 180.0).sin() * cell).sum::<f64>()
        / (4.0 * PI)
}

#[test]
fn a_lossless_antenna_radiates_its_input_power() {
    // MININEC conserves power only approximately (the dipole radiates
    // 0.998 of it); 2 % catches any mistake in the power or gain scaling
    for (name, p) in [("dipole", dipole()), ("yagi", yagi())] {
        let fraction = radiated_fraction(&p);
        assert!((fraction - 1.0).abs() < 0.02, "{name}: radiated {fraction} of the input power");
    }
    // over perfect ground all of it goes into the upper half
    let mut p = dipole();
    p.environment.ground = GroundKind::Perfect;
    p.environment.height = 10.0;
    let fraction = radiated_fraction(&p);
    assert!((fraction - 1.0).abs() < 0.02, "over perfect ground: radiated {fraction} of the input power");
}

#[test]
fn an_end_an_ulp_off_the_ground_is_on_it() {
    // the Ewe's down-lead set to a zenith angle of 180° in polar form ends
    // at -4e-16 m; MININEC asks for exactly 0 and left the antenna unfed
    let mut p = sample("EWE.MAA");
    let on = calc(&p);
    assert!(on.warnings.is_empty());
    for z in [-4.440892098500626e-16, 4.440892098500626e-16] {
        p.antenna.wires[2].end.z = z;
        let off = calc(&p);
        assert_eq!(off.sources, on.sources, "an end at {z} m");
        // (the pulses' positions are reported where the wire is)
        let currents = |r: &CalcResult| r.currents.iter().map(|c| c.current).collect::<Vec<_>>();
        assert_eq!(currents(&off), currents(&on), "an end at {z} m");
        assert_eq!(off.patterns, on.patterns, "an end at {z} m");
        assert!(off.warnings.is_empty(), "{:?}", off.warnings);
    }
    // a millimetre is a gap
    p.antenna.wires[2].end.z = 0.001;
    assert!(rel(calc(&p).impedance, on.impedance) > 0.5);

    // "an ulp" is a billionth of the wire's length: a 2 m vertical fed at
    // the ground, which is also the longest wire the warning measures by
    let mut p = project(vec![wire((0.0, 0.0, 0.0), (0.0, 0.0, 2.0), 10)], vec![source("w1b", 1.0, 0.0)]);
    p.environment.ground = GroundKind::Perfect;
    p.environment.height = 0.0;
    let on = calc(&p);
    p.antenna.wires[0].start.z = -1.5e-9;
    let off = calc(&p);
    assert_eq!(off.impedance, on.impedance);
    assert!(off.warnings.is_empty(), "{:?}", off.warnings);
    p.antenna.wires[0].start.z = -3e-9;
    assert_eq!(warned_wires(&p, WarningCode::BelowGround), Some(vec![1]));
}

#[test]
fn coordinates_an_ulp_apart_give_the_same_figures() {
    // on these grids a distance ratio lands on 1.1, 6 or 10, where MININEC
    // gives up the exact kernel or lowers the order of its integration; the
    // rounding of a coordinate chose the side, which moved |Z| by up to
    // 0.03 % for one coordinate an ulp off
    for name in ["LAZY-H.MAA", "VERT58.MAA", "MULTDP.MAA"] {
        let p = sample(name);
        let z = calc(&p).impedance;
        let coords = |w: &Wire| [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z];
        let ulp = p.antenna.wires.iter().flat_map(coords).fold(0.0, |m: f64, c| m.max(c.abs())) * f64::EPSILON;
        for (i, w) in p.antenna.wires.iter().enumerate() {
            for (k, at) in
                [w.start, w.end].into_iter().enumerate().flat_map(|(e, at)| (0..3).map(move |k| (e * 3 + k, at)))
            {
                for d in [-ulp, ulp] {
                    // every end at that point, so that joined wires stay joined
                    let mut q = p.clone();
                    for e in q.antenna.wires.iter_mut().flat_map(|w| [&mut w.start, &mut w.end]) {
                        if *e == at {
                            *[&mut e.x, &mut e.y, &mut e.z][k % 3] += d;
                        }
                    }
                    let off = rel(calc(&q).impedance, z);
                    assert!(off < 1e-9, "{name}: wire {} coordinate {k} {d:+e}: |Z| {off:e} apart", i + 1);
                }
            }
        }
    }
}

#[test]
fn nothing_radiates_below_the_ground() {
    // a horizontal cut 10° below the horizon: none over ground, the mirror
    // of the cut 10° above in free space
    let opts = CalcOptions { horizontal_cut_elevation_deg: Some(-10.0), ..CalcOptions::default() };
    let mut p = dipole();
    let free = calculate(&p, &opts).unwrap();
    assert!(free.patterns.horizontal.total.iter().all(|&g| g > -999.0));
    for ground in [GroundKind::Perfect, GroundKind::Real] {
        p.environment.ground = ground;
        p.environment.height = 10.0;
        let r = calculate(&p, &opts).unwrap();
        assert!(r.patterns.horizontal.total.iter().all(|&g| g == -999.0), "{ground:?}");
        // just above the horizon there is a field: on it, over perfect
        // ground, for a vertical wire (a horizontal one's image cancels it
        // there, as a real ground's reflection does for both)
        let mut vertical = project(vec![wire((0.0, 0.0, -5.2), (0.0, 0.0, 5.2), 20)], vec![source("w1c", 1.0, 0.0)]);
        vertical.environment.ground = ground;
        vertical.environment.height = 10.0;
        let solved = solve(&vertical, &CalcOptions::default()).unwrap();
        let zenith = if ground == GroundKind::Perfect { 90.0 } else { 89.0 };
        let near_horizon = solved.far_field((zenith, 0.0, 1), (0.0, 0.0, 1)).unwrap()[0];
        assert!(near_horizon.total_db > -999.0, "{ground:?}: {near_horizon:?}");
    }
}

#[test]
fn a_resistive_load_takes_what_is_not_radiated() {
    // P_radiated = P_in − |I|² R / 2, measured against the same antenna
    // without the load so MININEC's own 0.2 % shortfall cancels
    let lossless = radiated_fraction(&dipole());
    for (pulse, r_load) in [(5, 20.0), (10, 5.0), (15, 100.0)] {
        let mut p = dipole();
        p.antenna.loads_enabled = true;
        p.antenna.loads = vec![Load { pulse: pulse.to_string(), kind: LoadKind::Impedance { r: r_load, x: 0.0 } }];
        let r = calc(&p);
        let at_load = r.currents.iter().find(|c| c.pulse == pulse).unwrap().current;
        let in_load = abs(at_load).powi(2) * r_load / 2.0;
        let expected = (1.0 - in_load / r.total_power_w) * lossless;
        let fraction = radiated_fraction(&p);
        assert!(expected < 0.97, "pulse {pulse}: the load should take a good share: {expected}");
        assert!((fraction - expected).abs() < 0.003, "pulse {pulse}: radiated {fraction}, input less load {expected}");
    }
}

#[test]
fn ground_is_the_antenna_and_its_image_in_free_space() {
    // image theory: over perfect ground a wire at height h acts as itself
    // plus its mirror image at −h, with the horizontal current reversed and
    // the vertical current kept. Along the mirrored wire (its ends mirrored,
    // so a vertical one runs the other way) that is the opposite current.
    let h = 8.0;
    for (name, w) in [
        ("horizontal", wire((0.0, -5.2, 0.0), (0.0, 5.2, 0.0), 20)),
        ("vertical", wire((0.0, 0.0, -5.2), (0.0, 0.0, 5.2), 20)),
        ("sloping", wire((-2.0, -4.0, -3.0), (2.0, 4.0, 3.0), 20)),
    ] {
        let mut over_ground = project(vec![w.clone()], vec![source("w1c", 1.0, 0.0)]);
        over_ground.environment.ground = GroundKind::Perfect;
        over_ground.environment.height = h;
        let mut up = w.clone();
        up.start.z += h;
        up.end.z += h;
        let mut down = w.clone();
        down.start.z = -(w.start.z + h);
        down.end.z = -(w.end.z + h);
        let pair = project(vec![up, down], vec![source("w1c", 1.0, 0.0), source("w2c", 1.0, 180.0)]);
        let (a, b) = (calc(&over_ground), calc(&pair));
        // the kernel sums the image terms in another order: 1e-4 is well
        // inside the 4 significant digits MMANA shows
        let e = rel(a.sources[0].impedance, b.sources[0].impedance);
        assert!(
            e < 1e-4,
            "{name}: {:?} over ground, {:?} with the image",
            a.sources[0].impedance,
            b.sources[0].impedance
        );
    }
}

/// Feeds element 1 with element 2 shorted (a 0 V source) and the other way
/// round: I2/V1 and I1/V2, whose difference reciprocity makes 0.
fn reciprocity_error(first: Wire, second: Wire) -> f64 {
    let fed = |v1, v2| {
        let p = project(vec![first.clone(), second.clone()], vec![source("w1c", v1, 0.0), source("w2c", v2, 0.0)]);
        calc(&p)
    };
    rel(fed(1.0, 0.0).sources[1].current, fed(0.0, 1.0).sources[0].current)
}

#[test]
fn reciprocity_the_current_from_1_to_2_is_the_current_from_2_to_1() {
    // elements divided into the same segment lengths: equal to f32 rounding
    let e =
        reciprocity_error(wire((-2.0, -5.2, 0.0), (-2.0, 5.2, 0.0), 22), wire((2.0, -5.2, 1.0), (2.0, 5.2, 1.0), 22));
    assert!(e < 1e-6, "{e}");
    // otherwise MININEC's pulses are not quite symmetric; the difference is
    // discretisation error, so it falls about 4× each time the segments halve
    let unequal =
        |n| reciprocity_error(wire((-2.0, -5.5, 0.0), (-2.0, 5.5, 0.0), n), wire((2.0, -4.9, 0.0), (2.0, 4.9, 0.0), n));
    let (coarse, fine) = (unequal(22), unequal(44));
    assert!(coarse < 1e-3 && fine < coarse / 3.0, "{coarse} then {fine}");
}

#[test]
fn currents_add_up_source_by_source() {
    // the system is linear: both sources at once is the sum of each alone
    let both = {
        let mut p = yagi();
        p.antenna.sources = vec![source("w1c", 1.0, 0.0), source("w3c", 0.7, 40.0)];
        calc(&p)
    };
    let alone = |v1, v3| {
        let mut p = yagi();
        p.antenna.sources = vec![source("w1c", v1, 0.0), source("w3c", v3, 40.0)];
        calc(&p)
    };
    let (a, b) = (alone(1.0, 0.0), alone(0.0, 0.7));
    let peak = both.currents.iter().map(|c| abs(c.current)).fold(0.0, f64::max);
    for ((s, x), y) in both.currents.iter().zip(&a.currents).zip(&b.currents) {
        let sum = Complex { re: x.current.re + y.current.re, im: x.current.im + y.current.im };
        assert!(abs(sub(s.current, sum)) < 1e-5 * peak, "pulse {}", s.pulse);
    }
}

/// Two wires meeting at about 4°, with radii 16× apart at the junction
/// (found by random_models.rs). Each source alone takes power, both
/// together take less than none: MININEC breaks down here.
fn broken_down() -> Project {
    let mut p = project(
        vec![
            Wire {
                radius: -1.0,
                segmentation: Segmentation::TaperCustom { dm1: 412, dm2: None },
                ..wire((-0.96, 2.92, 0.0), (-3.0, 0.0, 0.0), 1)
            },
            Wire { radius: 0.0005, ..wire((0.0, 5.0, 0.0), (-3.0, 0.0, 0.0), 13) },
        ],
        vec![source("24", 1.0, 154.5), source("5", -1.0, 89.6)],
    );
    p.antenna.auto_segmentation.sc = 2.81;
    p.antenna.auto_segmentation.ec = 3;
    p.antenna.combined_wires = vec![mmana_core::CombinedWire {
        radius_key: -1.0,
        mode: 1,
        sections: vec![
            mmana_core::CombinedSection { length: 2.0, radius: 0.015 },
            mmana_core::CombinedSection { length: 0.0, radius: 0.008 },
        ],
    }];
    p
}

#[test]
fn a_result_that_takes_no_power_says_so() {
    let p = broken_down();
    let r = calc(&p);
    assert!(r.total_power_w < 0.0, "the case no longer breaks down: {}", r.total_power_w);
    assert!(r.warnings.iter().any(|w| w.message.starts_with("the input power is not positive")), "{:?}", r.warnings);
    // the near field is scaled by the input power: an error, not NaN
    let solved = solve(&p, &CalcOptions::default()).unwrap();
    let axis = mmana_core::AxisRange { start: 0.0, step: 0.0, count: 1 };
    let spec = mmana_core::NearFieldSpec {
        kind: mmana_core::NearFieldKind::Electric,
        power_w: 100.0,
        x: axis,
        y: axis,
        z: axis,
    };
    assert!(matches!(solved.near_field(&spec), Err(mmana_core::CalcError::NonPositivePower { .. })));
    // an ordinary antenna has no such warning
    assert!(calc(&yagi()).warnings.is_empty());
}

fn drift_warned(p: &Project) -> bool {
    calc(p).warnings.iter().any(|w| w.message.contains("segments: beyond about 250 on one conductor"))
}

/// The wires a warning of this kind names, if the result has one.
fn warned_wires(p: &Project, code: WarningCode) -> Option<Vec<usize>> {
    calc(p).warnings.into_iter().find(|w| w.code == code).map(|w| w.wires)
}

#[test]
fn warnings_name_the_wires_they_are_about() {
    // every wire of the conductor, and none of a separate one
    let halves = project(
        vec![
            wire((0.0, -5.2, 0.0), (0.0, 0.0, 0.0), 160),
            wire((5.0, -5.2, 0.0), (5.0, 5.2, 0.0), 20),
            wire((0.0, 0.0, 0.0), (0.0, 5.2, 0.0), 160),
        ],
        vec![source("160", 1.0, 0.0)],
    );
    assert_eq!(warned_wires(&halves, WarningCode::TooManySegments), Some(vec![1, 3]));
    // a stacked copy counts as the wire it copies
    let mut low = project(
        vec![wire((0.0, -5.2, 1.0), (0.0, 5.2, 1.0), 20), wire((3.0, -1.0, 1.0), (3.0, 1.0, -0.5), 10)],
        vec![source("w1c", 1.0, 0.0)],
    );
    low.environment.ground = GroundKind::Perfect;
    low.environment.height = 0.0;
    assert_eq!(warned_wires(&low, WarningCode::BelowGround), Some(vec![2]));
    // the height lifts every wire
    for w in &mut low.antenna.wires {
        w.start.z -= 1.0;
        w.end.z -= 1.0;
    }
    low.environment.height = 1.0;
    assert_eq!(warned_wires(&low, WarningCode::BelowGround), Some(vec![2]));
    low.antenna.stack.horizontal = 2;
    low.antenna.stack.horizontal_spacing = 8.0;
    assert_eq!(warned_wires(&low, WarningCode::BelowGround), Some(vec![2]));
}

#[test]
fn a_conductor_divided_too_finely_is_warned_about() {
    // the drift that segmentation_in_the_usable_range... stops short of
    let divided = |n| {
        let mut p = dipole();
        p.antenna.wires[0].segmentation = Segmentation::Fixed { count: n };
        p
    };
    assert!(!drift_warned(&divided(250)));
    assert!(drift_warned(&divided(320)));
    // a conductor counts whole however it is split into wires...
    let halves = project(
        vec![wire((0.0, -5.2, 0.0), (0.0, 0.0, 0.0), 160), wire((0.0, 0.0, 0.0), (0.0, 5.2, 0.0), 160)],
        vec![source("160", 1.0, 0.0)],
    );
    assert!(drift_warned(&halves));
    // ...and separate conductors do not add up, stacked copies included
    let apart = project(
        vec![wire((0.0, -5.2, 0.0), (0.0, 5.2, 0.0), 160), wire((5.0, -5.2, 0.0), (5.0, 5.2, 0.0), 160)],
        vec![source("w1c", 1.0, 0.0)],
    );
    assert!(!drift_warned(&apart));
    let mut stacked = divided(200);
    stacked.antenna.stack.horizontal = 3;
    assert!(!drift_warned(&stacked));
    // of the samples, only the two whose results move with the segmentation
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa");
    let mut warned: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| {
            calculate(&sample(name), &CalcOptions::default())
                .is_ok_and(|r| r.warnings.iter().any(|w| w.message.contains("on one conductor")))
        })
        .collect();
    warned.sort();
    assert_eq!(warned, ["CR2.MAA", "LPDA.MAA"]);
}

#[test]
fn moving_the_antenna_in_free_space_changes_nothing() {
    for (name, p) in [("dipole", dipole()), ("yagi", yagi())] {
        let mut q = p.clone();
        for w in &mut q.antenna.wires {
            for e in [&mut w.start, &mut w.end] {
                e.x += 3.7;
                e.y -= 1.3;
                e.z += 21.1;
            }
        }
        let (a, b) = (calc(&p), calc(&q));
        let e = rel(a.impedance, b.impedance);
        assert!(e < 1e-4, "{name}: {:?} moved to {:?}", a.impedance, b.impedance);
        assert!((a.gain_dbi - b.gain_dbi).abs() < 0.01, "{name}: {} vs {} dBi", a.gain_dbi, b.gain_dbi);
    }
}

#[test]
fn turning_the_antenna_turns_the_horizontal_pattern() {
    // 90° about Z swaps and negates coordinates, which is exact; the pattern
    // moves by 90 samples of 1°
    let p = yagi();
    let mut q = p.clone();
    for w in &mut q.antenna.wires {
        for e in [&mut w.start, &mut w.end] {
            (e.x, e.y) = (-e.y, e.x);
        }
    }
    let (a, b) = (calc(&p), calc(&q));
    assert!(rel(a.impedance, b.impedance) < 1e-5, "{:?} vs {:?}", a.impedance, b.impedance);
    let (ha, hb) = (&a.patterns.horizontal.total, &b.patterns.horizontal.total);
    let turned: Vec<f32> = (0..360).map(|i| ha[(i + 270) % 360]).collect();
    // samples near deep nulls swing by more; compare where the lobe is
    let peak = ha.iter().copied().fold(f32::MIN, f32::max);
    for (i, (x, y)) in turned.iter().zip(hb).enumerate() {
        if *x > peak - 30.0 {
            assert!((x - y).abs() < 0.01, "azimuth {i}: {x} vs {y}");
        }
    }
}

#[test]
fn scaling_size_and_wavelength_together_changes_nothing() {
    // with lossless wires only lengths in wavelengths matter
    for k in [2.0, 0.25] {
        for (name, p) in [("dipole", dipole()), ("yagi", yagi())] {
            let mut q = p.clone();
            q.antenna.frequency_mhz /= k;
            for w in &mut q.antenna.wires {
                w.radius *= k;
                for e in [&mut w.start, &mut w.end] {
                    (e.x, e.y, e.z) = (e.x * k, e.y * k, e.z * k);
                }
            }
            let (a, b) = (calc(&p), calc(&q));
            assert!(rel(a.impedance, b.impedance) < 1e-5, "{name} × {k}: {:?} vs {:?}", a.impedance, b.impedance);
            assert!((a.gain_dbi - b.gain_dbi).abs() < 0.001, "{name} × {k}");
        }
    }
}

#[test]
fn a_symmetric_antenna_has_a_symmetric_pattern() {
    // a dipole along Y radiates the same towards ±X and ±Y mirror images
    let r = calc(&dipole());
    let h = &r.patterns.horizontal.total;
    for i in 1..180 {
        assert!((h[i] - h[360 - i]).abs() < 1e-3, "azimuth {i} vs {}", 360 - i);
        assert!((h[i] - h[180 - i]).abs() < 1e-3, "azimuth {i} vs {}", 180 - i);
    }
}

#[test]
fn reversing_a_wire_does_not_change_its_feed_impedance() {
    let p = dipole();
    let mut q = p.clone();
    let w = &mut q.antenna.wires[0];
    std::mem::swap(&mut w.start, &mut w.end);
    let (a, b) = (calc(&p), calc(&q));
    assert!(rel(a.impedance, b.impedance) < 1e-5, "{:?} vs {:?}", a.impedance, b.impedance);
}

#[test]
fn segmentation_in_the_usable_range_barely_changes_the_impedance() {
    // from λ/40 to λ/330 per segment. Beyond about 250 segments on one
    // conductor the original drifts (docs/guarantees.md, "既知の限界"), so
    // this stops short of it
    let z: Vec<Complex> = [20, 40, 80, 160]
        .iter()
        .map(|&n| {
            let mut p = dipole();
            p.antenna.wires[0].segmentation = Segmentation::Fixed { count: n };
            calc(&p).impedance
        })
        .collect();
    let spread = |f: fn(&Complex) -> f64| {
        let v: Vec<f64> = z.iter().map(f).collect();
        v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min)
    };
    assert!(spread(|c| c.re) < 0.015 * z[0].re, "R: {z:?}");
    assert!(spread(|c| c.im) < 2.5, "X: {z:?}");
}

/// A folded dipole 10 cm wide, its long wires in `n` segments each.
fn folded(n: i32) -> Project {
    let (h, s) = (5.12, 0.1);
    project(
        vec![
            wire((0.0, -h, 0.0), (0.0, h, 0.0), n),
            wire((0.0, h, 0.0), (0.0, h, s), 1),
            wire((0.0, h, s), (0.0, -h, s), n),
            wire((0.0, -h, s), (0.0, -h, 0.0), 1),
        ],
        vec![source("w1c", 1.0, 0.0)],
    )
}

#[test]
fn a_point_calculated_as_lying_on_a_wire_beside_it_is_warned_about() {
    // 51 cm segments 10 cm apart: the resonance moves by 9 % (folded-segments.md)
    assert_eq!(warned_wires(&folded(20), WarningCode::CloseWires), Some(vec![1, 4]));
    // 39 cm: calculates within 0.1 %
    assert_eq!(warned_wires(&folded(26), WarningCode::CloseWires), None);
    // a sharp V is taken the same way at its apex, while its segments are long for the wavelength
    let v = |deg: f64, n: i32| {
        let h = (deg / 2.0).to_radians();
        let (y, z) = (5.12 * h.sin(), -5.12 * h.cos());
        project(
            vec![wire((0.0, 0.0, 0.0), (0.0, y, z), n), wire((0.0, 0.0, 0.0), (0.0, -y, z), n)],
            vec![source("w1b", 1.0, 0.0)],
        )
    };
    assert_eq!(warned_wires(&v(20.0, 20), WarningCode::CloseWires), Some(vec![1, 2]));
    assert_eq!(warned_wires(&v(20.0, 80), WarningCode::CloseWires), None);
    assert_eq!(warned_wires(&v(30.0, 20), WarningCode::CloseWires), None);
    // and no sample holding up under twice the segments is warned (samples.md)
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa");
    for name in ["HB9CVH.MAA", "DBLDP.MAA", "ZL20.MAA", "LPDA15.MAA", "MULTDP.MAA", "M2CQW.MAA"] {
        assert!(dir.join(name).exists());
        assert_eq!(warned_wires(&sample(name), WarningCode::CloseWires), None, "{name}");
    }
}
