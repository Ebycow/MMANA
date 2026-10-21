//! Small random models: connected and disabled wires, every kind of
//! segmentation, combined wires, sources and loads by any kind of name,
//! stacking and the three grounds. Whatever the model, the calculation must
//! end in time without a panic, and a result it returns must hold together.
//! The same goes for the sample `.maa` files with lines dropped, repeated,
//! cut short or given broken values.
//!
//! The default 256 cases run with `cargo test`; `PROPTEST_CASES=20000` runs
//! the long version (`.github/workflows/quality.yml` does so every night).
//! A failure is saved in `random_models.proptest-regressions` and replayed
//! first next time; once understood, it becomes an ordinary test in the file
//! it belongs to (input_ranges.rs, properties.rs...).

use mmana_core::{
    AutoSegmentation, AxisRange, CalcOptions, CalcResult, CombinedSection, CombinedWire, GroundKind, Load, LoadKind,
    NearFieldKind, NearFieldSpec, PatternResolution, Point3, Project, Segmentation, Source, Stack, StackDirection,
    Wire, WireMaterial, calculate, maa, pulse_layout, solve,
};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use serde_json::Value;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::OnceLock;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

/// Refuses any single allocation over 256 MB: a model or file that slips
/// past a limit then stops this test binary at once ("memory allocation of …
/// bytes failed") instead of filling memory and swap. The models here need a
/// few MB.
struct Capped;

const CAP: usize = 256 << 20;

unsafe impl GlobalAlloc for Capped {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > CAP { std::ptr::null_mut() } else { unsafe { System.alloc(layout) } }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.size() > CAP { std::ptr::null_mut() } else { unsafe { System.alloc_zeroed(layout) } }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > CAP { std::ptr::null_mut() } else { unsafe { System.realloc(ptr, layout, new_size) } }
    }
}

#[global_allocator]
static ALLOCATOR: Capped = Capped;

/// Generous for models of a few hundred pulses, which take milliseconds.
const LIMIT: Duration = Duration::from_secs(20);

/// Runs `f` on another thread so a calculation that never ends does not
/// hang the test; a panic is passed on as it is. A calculation still running
/// after [`LIMIT`] cannot be stopped, and shrinking would start more of them,
/// so the whole test binary ends there, printing the model to replay.
fn within<T: Send + 'static>(model: &Project, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(LIMIT) {
        Ok(v) => v,
        Err(RecvTimeoutError::Timeout) => {
            eprintln!("did not finish within {LIMIT:?}:\n{}", serde_json::to_string(model).unwrap());
            std::process::exit(101);
        }
        Err(RecvTimeoutError::Disconnected) => std::panic::resume_unwind(handle.join().unwrap_err()),
    }
}

fn point() -> impl Strategy<Value = Point3> {
    // mostly lattice points, so wires share ends and connect
    let lattice = (prop::sample::select(vec![-3.0, 0.0, 3.0]), prop::sample::select(vec![-5.0, 0.0, 5.0]), 0..2)
        .prop_map(|(x, y, z)| Point3::new(x, y, z as f64 * 4.0));
    let anywhere = (-6.0..6.0f64, -6.0..6.0f64, -2.0..6.0f64).prop_map(|(x, y, z)| Point3::new(x, y, z));
    prop_oneof![7 => lattice, 3 => anywhere]
}

fn segmentation() -> impl Strategy<Value = Segmentation> {
    prop_oneof![
        4 => (1..16i32).prop_map(|count| Segmentation::Fixed { count }),
        1 => Just(Segmentation::Auto),
        1 => Just(Segmentation::Taper),
        1 => Just(Segmentation::TaperStart),
        1 => Just(Segmentation::TaperEnd),
        1 => (50..800i32, prop::option::of(10..80i32)).prop_map(|(dm1, dm2)| Segmentation::TaperCustom { dm1, dm2 }),
    ]
}

fn wire() -> impl Strategy<Value = Wire> {
    // R = 0 disables the wire; -1 selects the combined wire
    let radius = prop_oneof![1 => Just(0.0), 1 => Just(-1.0), 8 => 0.0005..0.02f64];
    (point(), point(), radius, segmentation()).prop_map(|(start, end, radius, segmentation)| Wire {
        start,
        end,
        radius,
        segmentation,
        note: String::new(),
    })
}

fn pulse_name() -> impl Strategy<Value = String> {
    prop_oneof![
        (1..7usize, prop::sample::select(vec!["c", "b", "e"])).prop_map(|(w, k)| format!("w{w}{k}")),
        (1..7usize, prop::sample::select(vec!["b", "e", "c"]), -3..6i32).prop_map(|(w, k, n)| format!("w{w}{k}{n}")),
        (0..40usize).prop_map(|n| n.to_string()),
        Just("Ｗ１ｃ".to_owned()),
    ]
}

fn source() -> impl Strategy<Value = Source> {
    // with finite extremes, whose currents may not be representable
    let voltage = prop::sample::select(vec![0.0, 1.0, 0.5, -1.0, 1e-3, 1e-300, 1e30, 1e100, 1e300]);
    (pulse_name(), -180.0..180.0f64, voltage).prop_map(|(pulse, phase_deg, voltage)| Source {
        pulse,
        phase_deg,
        voltage,
    })
}

fn load() -> impl Strategy<Value = Load> {
    let value = prop::sample::select(vec![0.0, 0.1, 1.0, 100.0, -10.0, 1e-300, 1e300]);
    let kind = prop_oneof![
        (value.clone(), value.clone(), value.clone()).prop_map(|(l, c, q)| LoadKind::Lc {
            inductance_uh: l,
            capacitance_pf: c,
            q
        }),
        (value.clone(), value.clone()).prop_map(|(r, x)| LoadKind::Impedance { r, x }),
        prop::collection::vec((value.clone(), value), 0..4)
            .prop_map(|c| LoadKind::SDomain { coefficients: c.into_iter().map(|(a, b)| [a, b]).collect() }),
    ];
    (pulse_name(), kind).prop_map(|(pulse, kind)| Load { pulse, kind })
}

fn stack() -> impl Strategy<Value = Stack> {
    (
        1..4i32,
        1..3i32,
        prop::sample::select(vec![0.0, 0.5, 6.0, 12.0]),
        prop::sample::select(vec![0.0, 3.0, 10.0]),
        0..3,
    )
        .prop_map(|(horizontal, vertical, hs, vs, d)| Stack {
            horizontal,
            vertical,
            horizontal_spacing: hs,
            vertical_spacing: vs,
            direction: [StackDirection::Centered, StackDirection::Up, StackDirection::Down][d as usize],
        })
}

prop_compose! {
    fn model()(
        frequency in prop::sample::select(vec![0.01, 1.8, 7.05, 14.05, 28.5, 50.1, 145.0, 3000.0]),
        wires in prop::collection::vec(wire(), 1..6),
        sources in prop::collection::vec(source(), 0..4),
        loads in prop::collection::vec(load(), 0..3),
        loads_enabled in any::<bool>(),
        stack in prop_oneof![3 => Just(None), 1 => stack().prop_map(Some)],
        combined in any::<bool>(),
        environment in (0..3, prop::sample::select(vec![0.5, 5.0, 20.0, -1.0, 1e300]), 0..7),
        // everything far smaller or larger, up to coordinates of 1e150
        scale in prop::sample::select(vec![1.0, 1.0, 1.0, 1e-3, 1e4, 1e150]),
        auto in (prop::sample::select(vec![100, 400, 800]), prop::sample::select(vec![10, 40, 80]), 1.01..3.0f64, 1..4i32),
    ) -> Project {
        let (ground, height, material) = environment;
        let mut p = Project::default();
        let ant = &mut p.antenna;
        ant.frequency_mhz = frequency;
        ant.wires = wires;
        for e in ant.wires.iter_mut().flat_map(|w| [&mut w.start, &mut w.end]) {
            (e.x, e.y, e.z) = (e.x * scale, e.y * scale, e.z * scale);
        }
        ant.sources = sources;
        ant.loads = loads;
        ant.loads_enabled = loads_enabled;
        if let Some(stack) = stack {
            ant.stack = stack;
        }
        if combined {
            ant.combined_wires = vec![CombinedWire {
                radius_key: -1.0,
                mode: 1,
                sections: vec![CombinedSection { length: 2.0, radius: 0.015 }, CombinedSection { length: 0.0, radius: 0.008 }],
            }];
        }
        ant.auto_segmentation = AutoSegmentation { dm1: auto.0, dm2: auto.1, sc: auto.2, ec: auto.3 };
        p.environment.ground = GroundKind::from_code(ground);
        p.environment.height = height;
        p.environment.wire_material = WireMaterial::from_code(material);
        p
    }
}

/// One straight element of a model built to calculate: centred at `x`,
/// along Y tilted by `tilt` towards Z.
#[derive(Debug, Clone)]
struct Element {
    half_length: f64,
    tilt_deg: f64,
    /// Radius in wavelengths.
    radius: f64,
    segmentation: Segmentation,
}

fn element() -> impl Strategy<Value = Element> {
    let segmentation = prop_oneof![
        3 => (5..42i32).prop_map(|count| Segmentation::Fixed { count }),
        1 => Just(Segmentation::Auto),
        1 => Just(Segmentation::Taper),
    ];
    (0.1..0.6f64, -30.0..30.0f64, 1e-5..5e-4f64, segmentation).prop_map(
        |(half_length, tilt_deg, radius, segmentation)| Element { half_length, tilt_deg, radius, segmentation },
    )
}

prop_compose! {
    /// A model every calculation must accept: 1 to 4 straight elements at
    /// least 0.1 λ apart, each a few dozen segments at most, sources at the
    /// centres of different elements, loads with R ≥ 0, copies 0.6 λ apart
    /// or more, and everything above the ground.
    fn valid_model()(
        frequency in prop::sample::select(vec![3.5, 7.05, 14.05, 28.5, 50.1, 145.0]),
        elements in prop::collection::vec(element(), 1..5),
        spacing in 0.1..0.4f64,
        fed in prop::collection::vec((0..4usize, 0.1..10.0f64, -180.0..180.0f64), 1..3),
        load in prop::option::of((0..4usize, 0.0..1000.0f64, -1000.0..1000.0f64)),
        stack in prop::option::of((1..3i32, 1..3i32, 0.6..1.5f64)),
        environment in (0..3, 0.05..1.0f64, prop::sample::select(vec![0, 1, 3])),
    ) -> Project {
        let wavelength = 299.8 / frequency;
        let mut p = Project::default();
        let ant = &mut p.antenna;
        ant.frequency_mhz = frequency;
        let mut top = 0.0f64;
        for (i, e) in elements.iter().enumerate() {
            let (dy, dz) = (e.tilt_deg.to_radians().cos(), e.tilt_deg.to_radians().sin());
            let (h, x) = (e.half_length * wavelength, i as f64 * spacing * wavelength);
            top = top.max((h * dz).abs());
            ant.wires.push(Wire {
                start: Point3::new(x, -h * dy, -h * dz),
                end: Point3::new(x, h * dy, h * dz),
                radius: e.radius * wavelength,
                segmentation: e.segmentation,
                note: String::new(),
            });
        }
        let n = elements.len();
        let mut used = Vec::new();
        for (k, voltage, phase_deg) in fed {
            if !used.contains(&(k % n)) {
                used.push(k % n);
                ant.sources.push(Source { pulse: format!("w{}c", k % n + 1), phase_deg, voltage });
            }
        }
        if let Some((k, r, x)) = load {
            ant.loads_enabled = true;
            ant.loads = vec![Load { pulse: format!("w{}b1", k % n + 1), kind: LoadKind::Impedance { r, x } }];
        }
        let mut height_needed = top;
        if let Some((horizontal, vertical, apart)) = stack {
            ant.stack = Stack {
                horizontal,
                vertical,
                horizontal_spacing: apart * wavelength,
                vertical_spacing: apart * wavelength,
                direction: StackDirection::Up,
            };
        }
        let (ground, clearance, material) = environment;
        height_needed += clearance * wavelength;
        p.environment.ground = GroundKind::from_code(ground);
        p.environment.height = height_needed;
        p.environment.wire_material = WireMaterial::from_code(material);
        p
    }
}

/// A scan axis of up to 3 samples, with finite extremes.
fn axis() -> impl Strategy<Value = AxisRange> {
    (
        prop::sample::select(vec![0.0, 45.0, -90.0, 1.0, 1e300, -1e300, 1e308]),
        prop::sample::select(vec![0.0, 1.0, 15.0, -0.1, 1e300, 1e308, -1e308]),
        -1..4i32,
    )
        .prop_map(|(start, step, count)| AxisRange { start, step, count })
}

fn power() -> impl Strategy<Value = f64> {
    prop::sample::select(vec![100.0, 1e-3, 1e-300, 1e300, 1e308, 0.0, -1.0])
}

fn options() -> impl Strategy<Value = CalcOptions> {
    (any::<bool>(), any::<bool>(), prop::option::of(-90.0..=90.0f64)).prop_map(|(coarse, fix, cut)| CalcOptions {
        pattern_resolution: if coarse { PatternResolution::Coarse } else { PatternResolution::Fine },
        fix_free_space_elevation: fix,
        horizontal_cut_elevation_deg: cut,
        // keeps every case to milliseconds
        max_pulses: 600,
        ..CalcOptions::default()
    })
}

/// Paths of the nulls in a JSON value: NaN and infinities serialise as null.
fn nulls(v: &Value, path: &str, out: &mut Vec<String>) {
    match v {
        Value::Null => out.push(path.to_owned()),
        Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| nulls(x, &format!("{path}[{i}]"), out)),
        Value::Object(o) => o.iter().for_each(|(k, x)| nulls(x, &format!("{path}.{k}"), out)),
        _ => {}
    }
}

fn no_nulls(v: impl serde::Serialize, what: &str) -> Result<(), TestCaseError> {
    let mut found = Vec::new();
    nulls(&serde_json::to_value(v).unwrap(), "", &mut found);
    prop_assert!(found.is_empty(), "{what} has non-finite values at {:?}", &found[..found.len().min(5)]);
    Ok(())
}

/// What a successful calculation must satisfy whatever the model.
fn holds_together(p: &Project, o: &CalcOptions, r: &CalcResult) -> Result<(), TestCaseError> {
    no_nulls(r, "the result")?;
    let n = r.pulse_count;
    prop_assert!(n >= 1 && n <= o.max_pulses);
    prop_assert_eq!(r.currents.len(), n);
    // the original's display list, which may stop before the last pulse
    prop_assert!(r.current_display.len() <= n);
    prop_assert!(r.currents.iter().enumerate().all(|(i, c)| c.pulse == i + 1));
    let copies = (p.antenna.stack.horizontal * p.antenna.stack.vertical).max(1) as usize;
    prop_assert_eq!(r.sources.len(), p.antenna.sources.len() * copies);
    for s in &r.sources {
        prop_assert!((1..=n).contains(&s.pulse), "source on pulse {} of {}", s.pulse, n);
        prop_assert!(s.swr >= 1.0, "SWR {}", s.swr);
    }
    let sum: f64 = r.sources.iter().map(|s| s.power_w).sum();
    prop_assert!((r.total_power_w - sum).abs() <= 1e-9 * sum.abs().max(1e-30));
    // a result that takes no power is flagged
    if r.total_power_w <= 0.0 {
        prop_assert!(r.warnings.iter().any(|w| w.starts_with("the input power is not positive")));
    }
    let (nh, nv) = match o.pattern_resolution {
        PatternResolution::Fine => (360, 181),
        PatternResolution::Coarse => (180, 91),
    };
    prop_assert_eq!(r.patterns.horizontal.total.len(), nh);
    prop_assert_eq!(r.patterns.vertical.total.len(), nv);
    let wires = p.antenna.wires.len() * copies;
    prop_assert!(r.segmented_wires.iter().all(|w| (1..=wires).contains(&w.wire)));
    Ok(())
}

/// The sample files as text, one string each.
fn samples() -> &'static [String] {
    static SAMPLES: OnceLock<Vec<String>> = OnceLock::new();
    SAMPLES.get_or_init(|| {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa");
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        paths.iter().map(|p| maa::decode(&std::fs::read(p).unwrap())).collect()
    })
}

#[derive(Debug, Clone)]
enum Edit {
    Drop(usize),
    Repeat(usize),
    Insert(usize, &'static str),
    /// Replaces one comma-separated field of a line.
    Field(usize, usize, &'static str),
    CutAt(usize),
}

/// Values a hand-edited or damaged file may hold.
const BROKEN: [&str; 16] = [
    "",
    "0",
    "-1",
    "1e999",
    "-1e999",
    "99999999999",
    "-2147483648",
    "1e-300",
    "nan",
    "1/0",
    "abc",
    ",,,,",
    "w99c",
    "*** Wires ***",
    "１２",
    "0.0000001",
];

fn edit() -> impl Strategy<Value = Edit> {
    let at = 0..60usize;
    let value = prop::sample::select(BROKEN.to_vec());
    prop_oneof![
        1 => at.clone().prop_map(Edit::Drop),
        1 => at.clone().prop_map(Edit::Repeat),
        1 => (at.clone(), value.clone()).prop_map(|(l, v)| Edit::Insert(l, v)),
        3 => (at.clone(), 0..9usize, value).prop_map(|(l, f, v)| Edit::Field(l, f, v)),
        1 => at.prop_map(Edit::CutAt),
    ]
}

fn damaged(sample: usize, edits: &[Edit]) -> String {
    let text = &samples()[sample % samples().len()];
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for e in edits {
        let n = lines.len().max(1);
        match *e {
            Edit::Drop(l) if !lines.is_empty() => {
                lines.remove(l % n);
            }
            Edit::Repeat(l) if !lines.is_empty() => {
                let line = lines[l % n].clone();
                lines.insert(l % n, line);
            }
            Edit::Insert(l, v) => lines.insert(l % (lines.len() + 1), v.to_owned()),
            Edit::Field(l, f, v) if !lines.is_empty() => {
                let line = &mut lines[l % n];
                let mut fields: Vec<String> = line.split(',').map(str::to_owned).collect();
                let k = f % fields.len();
                fields[k] = v.to_owned();
                *line = fields.join(",");
            }
            Edit::CutAt(l) => lines.truncate(l % (lines.len() + 1)),
            _ => {}
        }
    }
    lines.join("\r\n")
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn any_model_ends_in_time_and_a_result_holds_together(p in model(), o in options()) {
        let (q, oo) = (p.clone(), o.clone());
        let (result, layout) = within(&p, move || (calculate(&q, &oo), pulse_layout(&q, &oo)));
        match (&result, &layout) {
            (Ok(r), Ok(l)) => {
                holds_together(&p, &o, r)?;
                no_nulls(&l.wires, "the layout")?;
                prop_assert_eq!(l.pulse_count, r.pulse_count);
                // every source of the antenna as defined has a place
                prop_assert!(l.sources.iter().all(Option::is_some), "sources {:?}", l.sources);
                if p.antenna.loads_enabled {
                    prop_assert!(l.loads.iter().all(Option::is_some), "loads {:?}", l.loads);
                }
                // in order; a pulse at a junction is listed under both wires
                let pulses: Vec<usize> = l.wires.iter().flat_map(|w| w.pulses.iter().map(|q| q.pulse)).collect();
                prop_assert!(pulses.windows(2).all(|w| w[0] <= w[1]), "{:?}", pulses);
                // and every name listed addresses its pulse
                if !p.antenna.stack.is_stacked() {
                    for q in l.wires.iter().flat_map(|w| &w.pulses) {
                        let mut named = p.clone();
                        named.antenna.sources = vec![Source { pulse: q.name.clone(), phase_deg: 0.0, voltage: 1.0 }];
                        let at = pulse_layout(&named, &o).map_err(|e| TestCaseError::fail(e.to_string()))?.sources[0];
                        prop_assert_eq!(at, Some(q.position), "{} (pulse {})", &q.name, q.pulse);
                    }
                }
            }
            // the layout checks less than the calculation, never more
            (Ok(_), Err(e)) => prop_assert!(false, "calculated, but the layout failed with {e}"),
            (Err(_), _) => {}
        }
    }

    #[test]
    fn field_scans_of_any_model_are_finite(
        p in model(),
        far in (axis(), axis()),
        near in (axis(), axis(), axis(), power()),
    ) {
        let q = p.clone();
        let scans = within(&p, move || {
            let solved = solve(&q, &CalcOptions { max_pulses: 600, ..CalcOptions::default() }).ok()?;
            let (x, y, z, power_w) = near;
            let spec = NearFieldSpec { kind: NearFieldKind::Electric, power_w, x, y, z };
            Some((
                solved.far_field((far.0.start, far.0.step, far.0.count), (far.1.start, far.1.step, far.1.count)),
                solved.near_field(&spec),
                solved.near_field(&NearFieldSpec { kind: NearFieldKind::Magnetic, ..spec }),
            ))
        });
        // each scan that succeeds is checked, whatever the others did
        if let Some((far, electric, magnetic)) = scans {
            if let Ok(far) = far {
                no_nulls(&far, "the far field")?;
            }
            if let Ok(electric) = electric {
                no_nulls(&electric, "the electric near field")?;
            }
            if let Ok(magnetic) = magnetic {
                no_nulls(&magnetic, "the magnetic near field")?;
            }
        }
    }

    #[test]
    fn a_maa_file_read_back_reads_back_the_same(p in model()) {
        // writing rounds to 6 decimals, so the first round trip may change
        // values; from then on the file must be stable
        let once = maa::parse(&maa::write(&p)).project;
        let twice = maa::parse(&maa::write(&once)).project;
        prop_assert_eq!(&twice, &once);
        prop_assert_eq!(maa::write(&twice), maa::write(&once));
    }

    #[test]
    fn project_json_round_trips_exactly(p in model()) {
        let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        prop_assert_eq!(back, p);
    }

    #[test]
    fn a_damaged_maa_file_reads_and_calculates_without_a_panic(
        sample in 0..1000usize,
        edits in prop::collection::vec(edit(), 1..6),
    ) {
        let text = damaged(sample, &edits);
        let p = maa::parse(&text).project;
        let o = CalcOptions { max_pulses: 600, ..CalcOptions::default() };
        let (q, oo) = (p.clone(), o.clone());
        let (result, layout) = within(&p, move || (calculate(&q, &oo), pulse_layout(&q, &oo)));
        if let Ok(r) = &result {
            holds_together(&p, &o, r)?;
            prop_assert!(layout.is_ok(), "calculated, but the layout failed: {:?}", layout.err());
        }
        // what was read writes back stably
        let once = maa::parse(&maa::write(&p)).project;
        prop_assert_eq!(&maa::parse(&maa::write(&once)).project, &once);
    }

    #[test]
    fn a_model_built_to_calculate_calculates(p in valid_model()) {
        // about 1000 pulses at most, more at the top of the sweep
        let o = CalcOptions::default();
        let (q, oo) = (p.clone(), o.clone());
        let (result, layout, scans, sweep) = within(&p, move || {
            let solved = solve(&q, &oo);
            let scans = solved.as_ref().ok().map(|s| {
                let axis = |start, step, count| AxisRange { start, step, count };
                let spec = NearFieldSpec { kind: NearFieldKind::Electric, power_w: 100.0, x: axis(-1.0, 2.0, 2), y: axis(0.5, 0.0, 1), z: axis(3.0, 1.0, 2) };
                (s.far_field((0.0, 30.0, 7), (0.0, 60.0, 6)), s.near_field(&spec), s.near_field(&NearFieldSpec { kind: NearFieldKind::Magnetic, ..spec }))
            });
            let f = q.antenna.frequency_mhz;
            (calculate(&q, &oo), pulse_layout(&q, &oo), scans, mmana_core::sweep(&q, &oo, 0.97 * f, 1.03 * f, 3))
        });
        let r = result.map_err(|e| TestCaseError::fail(format!("rejected: {e}")))?;
        holds_together(&p, &o, &r)?;
        prop_assert!(r.total_power_w > 0.0, "input power {}", r.total_power_w);
        prop_assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        prop_assert!(r.gain_dbi > -999.0);
        let l = layout.map_err(|e| TestCaseError::fail(format!("layout rejected: {e}")))?;
        prop_assert!(l.sources.iter().all(Option::is_some));
        let (far, electric, magnetic) = scans.unwrap();
        no_nulls(far.map_err(|e| TestCaseError::fail(e.to_string()))?, "the far field")?;
        no_nulls(electric.map_err(|e| TestCaseError::fail(e.to_string()))?, "the electric near field")?;
        no_nulls(magnetic.map_err(|e| TestCaseError::fail(e.to_string()))?, "the magnetic near field")?;
        let points = sweep.map_err(|e| TestCaseError::fail(format!("sweep rejected: {e}")))?;
        prop_assert_eq!(points.len(), 3);
        no_nulls(&points, "the sweep")?;
    }
}
