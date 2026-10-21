//! Small random models: connected and disabled wires, every kind of
//! segmentation, combined wires, sources and loads by any kind of name,
//! stacking and the three grounds. Whatever the model, the calculation must
//! end in time without a panic, and a result it returns must hold together.
//!
//! The default 256 cases run with `cargo test`; `PROPTEST_CASES=20000` runs
//! the long version (`.github/workflows/quality.yml` does so every night).
//! A failure is saved in `random_models.proptest-regressions` and replayed
//! first next time; once understood, it becomes an ordinary test in the file
//! it belongs to (input_ranges.rs, properties.rs...).

use mmana_core::{
    AutoSegmentation, AxisRange, CalcError, CalcOptions, CalcResult, CombinedSection, CombinedWire, GroundKind, Load,
    LoadKind, NearFieldKind, NearFieldSpec, PatternResolution, Point3, Project, Segmentation, Source, Stack,
    StackDirection, Wire, WireMaterial, calculate, maa, pulse_layout, solve,
};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use serde_json::Value;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

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
    (pulse_name(), -180.0..180.0f64, prop::sample::select(vec![0.0, 1.0, 0.5, -1.0, 1e-3]))
        .prop_map(|(pulse, phase_deg, voltage)| Source { pulse, phase_deg, voltage })
}

fn load() -> impl Strategy<Value = Load> {
    let value = prop::sample::select(vec![0.0, 0.1, 1.0, 100.0, -10.0]);
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
        frequency in prop::sample::select(vec![1.8, 7.05, 14.05, 28.5, 50.1, 145.0]),
        wires in prop::collection::vec(wire(), 1..6),
        sources in prop::collection::vec(source(), 0..4),
        loads in prop::collection::vec(load(), 0..3),
        loads_enabled in any::<bool>(),
        stack in prop_oneof![3 => Just(None), 1 => stack().prop_map(Some)],
        combined in any::<bool>(),
        ground in 0..3,
        height in prop::sample::select(vec![0.5, 5.0, 20.0, -1.0]),
        material in 0..7,
        auto in (prop::sample::select(vec![100, 400, 800]), prop::sample::select(vec![10, 40, 80]), 1.01..3.0f64, 1..4i32),
    ) -> Project {
        let mut p = Project::default();
        let ant = &mut p.antenna;
        ant.frequency_mhz = frequency;
        ant.wires = wires;
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
    fn field_scans_of_any_model_are_finite(p in model()) {
        let q = p.clone();
        let scans = within(&p, move || {
            let solved = solve(&q, &CalcOptions { max_pulses: 600, ..CalcOptions::default() })?;
            let far = solved.far_field((0.0, 45.0, 5), (0.0, 90.0, 4))?;
            let axis = |start| AxisRange { start, step: 1.5, count: 2 };
            let spec = NearFieldSpec { kind: NearFieldKind::Electric, power_w: 100.0, x: axis(-1.0), y: axis(0.3), z: axis(2.0) };
            let near = solved.near_field(&spec)?;
            let magnetic = solved.near_field(&NearFieldSpec { kind: NearFieldKind::Magnetic, ..spec })?;
            Ok::<_, CalcError>((far, near, magnetic))
        });
        if let Ok((far, near, magnetic)) = scans {
            no_nulls(&far, "the far field")?;
            no_nulls(&near, "the electric near field")?;
            no_nulls(&magnetic, "the magnetic near field")?;
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
}
