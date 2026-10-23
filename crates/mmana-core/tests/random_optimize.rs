//! Random optimisations of small antennas: variables made by random edits
//! of the setup, random goals, weights, bands and pitches. Whatever the
//! setup, the optimiser must not panic and must end in time; when it runs,
//! the best trial is no worse than the start, and every trial it lists
//! gives, written back with `apply_trial`, the antenna whose figures it
//! lists.
//!
//! The default 256 cases run with `cargo test`; `PROPTEST_CASES=20000` runs
//! the long version (`.github/workflows/quality.yml` does so every night).
//! Failures are saved in `random_optimize.proptest-regressions`.

use mmana_core::optimize::{
    self, Band, ElementPart, Goals, LoadPart, Matching, OptimizerSetup, PolarAnchor, PolarQuantity, Progress, SetupOp,
    SourcePart, StackPart, Trial, VariableTarget, Weights, WirePart,
};
use mmana_core::{
    CalcOptions, GroundKind, Load, LoadKind, PatternResolution, Point3, Project, Segmentation, Source, Wire, calculate,
    maa,
};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::OnceLock;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

/// Refuses any single allocation over 256 MB (see random_models.rs).
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

const LIMIT: Duration = Duration::from_secs(60);

/// Calculations per optimisation: enough for a few rounds of a few variables.
const CALCULATIONS: usize = 40;

/// Runs `f` on another thread; after [`LIMIT`] the test binary ends, printing
/// `what` to replay (see random_models.rs).
fn within<T: Send + 'static>(what: impl serde::Serialize, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(LIMIT) {
        Ok(v) => v,
        Err(RecvTimeoutError::Timeout) => {
            eprintln!("did not finish within {LIMIT:?}:\n{}", serde_json::to_string(&what).unwrap());
            std::process::exit(101);
        }
        Err(RecvTimeoutError::Disconnected) => std::panic::resume_unwind(handle.join().unwrap_err()),
    }
}

/// The sample antennas of at most 120 pulses, which calculate as they are.
fn small_samples() -> &'static [Project] {
    static SAMPLES: OnceLock<Vec<Project>> = OnceLock::new();
    SAMPLES.get_or_init(|| {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa");
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        let options = CalcOptions { max_pulses: 120, ..CalcOptions::default() };
        paths
            .iter()
            .map(|p| maa::parse_bytes(&std::fs::read(p).unwrap()).project)
            .filter(|p| calculate(p, &options).is_ok_and(|r| r.warnings.is_empty()))
            .collect()
    })
}

prop_compose! {
    /// A Yagi of 2 to 4 straight elements along X, fed at the centre of the
    /// second (or the only driven) element, a few segments each.
    fn yagi()(
        frequency in prop::sample::select(vec![7.05, 14.05, 50.1, 145.0]),
        lengths in prop::collection::vec(0.42..0.52f64, 2..5),
        spacing in 0.1..0.3f64,
        segments in 5..12i32,
        load in prop::option::of((0..4usize, 0.0..50.0f64, -200.0..200.0f64, any::<bool>())),
        ground in (0..3, 0.3..1.0f64),
    ) -> Project {
        let wavelength = 299.8 / frequency;
        let mut p = Project::default();
        let ant = &mut p.antenna;
        ant.frequency_mhz = frequency;
        for (i, l) in lengths.iter().enumerate() {
            let (h, x) = (l * wavelength / 2.0, i as f64 * spacing * wavelength);
            ant.wires.push(Wire {
                start: Point3::new(x, -h, 0.0),
                end: Point3::new(x, h, 0.0),
                radius: 0.001 * wavelength,
                segmentation: Segmentation::Fixed { count: segments | 1 },
                note: String::new(),
            });
        }
        ant.sources = vec![Source { pulse: "w2c".into(), phase_deg: 0.0, voltage: 1.0 }];
        if let Some((k, r, x, lc)) = load {
            ant.loads_enabled = true;
            let kind = if lc {
                LoadKind::Lc { inductance_uh: 1.0 + r / 10.0, capacitance_pf: 0.0, q: 0.0 }
            } else {
                LoadKind::Impedance { r, x }
            };
            ant.loads = vec![Load { pulse: format!("w{}c", k % lengths.len() + 1), kind }];
        }
        p.environment.ground = GroundKind::from_code(ground.0);
        p.environment.height = ground.1 * wavelength;
        p
    }
}

fn project() -> impl Strategy<Value = Project> {
    prop_oneof![
        3 => yagi(),
        1 => (0..small_samples().len()).prop_map(|i| small_samples()[i].clone()),
    ]
}

fn target() -> impl Strategy<Value = VariableTarget> {
    use VariableTarget as T;
    let n = 0..6usize;
    let wire_part = prop::sample::select(vec![
        WirePart::Y1,
        WirePart::Z1,
        WirePart::Y2,
        WirePart::Z2,
        WirePart::Radius,
        WirePart::X1,
        WirePart::X2,
    ]);
    let element_part = prop::sample::select(vec![
        ElementPart::Interval,
        ElementPart::Width,
        ElementPart::Height,
        ElementPart::Perimeter,
        ElementPart::Radius,
        ElementPart::Position,
        ElementPart::WidthAll,
        ElementPart::HeightAll,
        ElementPart::PerimeterAll,
    ]);
    let quantity = prop::sample::select(vec![PolarQuantity::Length, PolarQuantity::Azimuth, PolarQuantity::Zenith]);
    let anchor = prop::sample::select(vec![PolarAnchor::Start, PolarAnchor::End, PolarAnchor::Center]);
    prop_oneof![
        3 => (n.clone(), wire_part).prop_map(|(wire, part)| T::Wire { wire, part }),
        1 => (n.clone(), quantity, anchor).prop_map(|(wire, quantity, anchor)| T::PolarWire { wire, quantity, anchor }),
        4 => (n.clone(), element_part).prop_map(|(element, part)| T::Element { element, part }),
        1 => (n.clone(), prop::sample::select(vec![LoadPart::Primary, LoadPart::Secondary]))
            .prop_map(|(load, part)| T::Load { load, part }),
        1 => Just(T::Height),
        1 => Just(T::Frequency),
        1 => (n, prop::sample::select(vec![SourcePart::Phase, SourcePart::Voltage]))
            .prop_map(|(source, part)| T::Source { source, part }),
        1 => prop::sample::select(vec![StackPart::Both, StackPart::Horizontal, StackPart::Vertical])
            .prop_map(|part| T::Stack { part }),
    ]
}

fn setup_op() -> impl Strategy<Value = SetupOp> {
    let index = 0..8usize;
    prop_oneof![
        6 => (index.clone(), target()).prop_map(|(index, target)| SetupOp::SetTarget { index, target }),
        2 => (any::<bool>(), any::<bool>()).prop_map(|(intervals, scale_all)| SetupOp::AllElements { intervals, scale_all, combination: None }),
        1 => index.clone().prop_map(|index| SetupOp::LinkSame { index }),
        1 => Just(SetupOp::LinkElements),
        1 => index.clone().prop_map(|index| SetupOp::Insert { index }),
        1 => index.prop_map(|index| SetupOp::Delete { index }),
    ]
}

fn weights() -> impl Strategy<Value = Weights> {
    prop::array::uniform7(prop_oneof![2 => Just(0u32), 1 => 1..101u32]).prop_map(
        |[gain, fb, jx, swr, elevation, matching, current]| Weights { gain, fb, jx, swr, elevation, matching, current },
    )
}

fn goals() -> impl Strategy<Value = Goals> {
    let matching = prop_oneof![
        Just(Matching::Capacitance),
        Just(Matching::Hairpin),
        (prop::option::of(10.0..100.0f64), prop::option::of(-50.0..50.0f64))
            .prop_map(|(r, x)| Matching::Impedance { r, x }),
    ];
    (
        prop::sample::select(vec![100.0, 10.0, 6.0]),
        prop::sample::select(vec![25.0, 15.0]),
        prop::sample::select(vec![0.0, 5.0]),
        prop::sample::select(vec![1.0, 1.5]),
        matching,
        prop::sample::select(vec!["w1c", "w2c", "w3b", ""]),
        any::<bool>(),
    )
        .prop_map(|(max_gain_dbi, max_fb_db, min_jx, min_swr, matching, pulse, minimize_current)| Goals {
            max_gain_dbi,
            max_fb_db,
            min_jx,
            min_swr,
            matching,
            current_pulse: pulse.into(),
            minimize_current,
        })
}

#[derive(Debug, Clone)]
struct Case {
    project: Project,
    options: CalcOptions,
    setup: OptimizerSetup,
}

prop_compose! {
    fn case()(
        project in project(),
        // the first edit makes a variable
        first in prop_oneof![
            3 => target().prop_map(|target| SetupOp::SetTarget { index: 0, target }),
            1 => (any::<bool>(), any::<bool>()).prop_map(|(intervals, scale_all)| SetupOp::AllElements { intervals, scale_all, combination: None }),
        ],
        more in prop::collection::vec(setup_op(), 0..5),
        weights in weights(),
        goals in goals(),
        sweep_only in prop::bool::weighted(0.15),
        absolute_pitch in any::<bool>(),
        // relative to the pitch MMANA proposes
        pitch_scale in prop::sample::select(vec![1.0, 1.0, 0.1, 5.0]),
        bands in prop_oneof![3 => Just(Vec::new()), 1 => prop::collection::vec(0.97..1.03f64, 2..4)],
        emphasize_first_band in any::<bool>(),
        start_frequency in prop_oneof![3 => Just(None), 1 => (0.98..1.02f64).prop_map(Some)],
    ) -> Case {
        let f = project.antenna.frequency_mhz;
        let options = CalcOptions {
            frequency_mhz: start_frequency.map(|k| k * f),
            max_pulses: 400,
            ..CalcOptions::default()
        };
        let mut setup = OptimizerSetup {
            weights,
            goals,
            sweep_only,
            absolute_pitch,
            coarse_pattern: true,
            emphasize_first_band,
            ..OptimizerSetup::default()
        };
        let ops: Vec<SetupOp> = std::iter::once(first).chain(more).collect();
        setup = optimize::edit_setup(&project, &options, &setup, &ops).setup;
        for v in &mut setup.variables {
            v.pitch *= pitch_scale;
        }
        setup.bands = bands
            .iter()
            .map(|k| Band { frequency_mhz: k * f, sources: project.antenna.sources.clone() })
            .collect();
        Case { project, options, setup }
    }
}

/// The figures of a trial: band 1 as `Progress` and `Trial` keep them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Figures {
    r: f32,
    jx: f32,
    swr: f32,
    gain: f32,
    fb: f32,
    elevation: f32,
}

impl Figures {
    fn of_trial(t: &Trial) -> Self {
        Self {
            r: t.impedance.re as f32,
            jx: t.impedance.im as f32,
            swr: t.swr,
            gain: t.gain_dbi,
            fb: t.fb_db,
            elevation: t.elevation_deg,
        }
    }

    fn of_progress(p: &Progress) -> Self {
        Self {
            r: p.impedance.re as f32,
            jx: p.impedance.im as f32,
            swr: p.swr,
            gain: p.gain_dbi,
            fb: p.fb_db,
            elevation: p.elevation_deg,
        }
    }

    /// The same to what matters when building: 0.1 % of |Z|, 0.001 of |Γ|,
    /// 0.05 dB. A trial
    /// reaches its values one step at a time and `apply_trial` sets them at
    /// once; scaling an element again and again rounds its coordinates
    /// differently, which grows with the steps (about 0.015 % of |Z| after
    /// 850 calculations on the sensitive Feeder TEST sample).
    fn close(&self, o: &Self) -> bool {
        let z = self.r.hypot(self.jx).max(o.r.hypot(o.jx));
        (self.r - o.r).hypot(self.jx - o.jx) <= 0.01 + 1e-3 * z
            // (as |Γ|: an SWR of 129 is as good as one of 129.5)
            && (gamma(self.swr) - gamma(o.swr)).abs() <= 1e-3
            && (self.gain - o.gain).abs() <= 0.05
            && (self.fb - o.fb).abs() <= 0.05
            && (self.elevation - o.elevation).abs() <= 2.0
    }

    /// What a calculation of `project` at `frequency_mhz` gives, and whether
    /// the antenna takes power the calculation can tell from rounding.
    fn calculated(project: &Project, options: &CalcOptions, frequency_mhz: f64) -> Result<(Self, bool), String> {
        let options = CalcOptions {
            frequency_mhz: Some(frequency_mhz),
            pattern_resolution: PatternResolution::Coarse,
            ..options.clone()
        };
        let r = calculate(project, &options).map_err(|e| e.to_string())?;
        let figures = Self {
            r: r.impedance.re as f32,
            jx: r.impedance.im as f32,
            swr: r.swr as f32,
            gain: r.gain_dbi,
            fb: r.fb_db,
            elevation: r.elevation_deg,
        };
        Ok((figures, !r.warnings.iter().any(|w| w.message.starts_with("the input power"))))
    }
}

/// The same project but for coordinates and values within 1 ppm.
fn same_to_rounding(a: &Project, b: &Project) -> bool {
    let mut x = serde_json::to_value(a).unwrap();
    let y = serde_json::to_value(b).unwrap();
    fn close(x: &mut serde_json::Value, y: &serde_json::Value) -> bool {
        use serde_json::Value::*;
        match (x, y) {
            (Number(a), Number(b)) => {
                let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
            }
            (Array(a), Array(b)) => a.len() == b.len() && a.iter_mut().zip(b).all(|(a, b)| close(a, b)),
            (Object(a), Object(b)) => {
                a.len() == b.len() && a.iter_mut().all(|(k, v)| b.get(k).is_some_and(|w| close(v, w)))
            }
            (a, b) => a == b,
        }
    }
    close(&mut x, &y)
}

/// The reflection coefficient |Γ| of an SWR.
fn gamma(swr: f32) -> f32 {
    (swr - 1.0) / (swr + 1.0)
}

/// Band 1 of a run: the antenna's own sources and frequency, or the first
/// band's when the bands are scored.
fn band_one(case: &Case, project: &Project, frequency_mhz: f64) -> (Project, f64) {
    let s = &case.setup;
    let banded = !s.sweep_only && case.project.antenna.sources.len() <= 4 && s.bands.len() >= 2;
    if !banded {
        return (project.clone(), frequency_mhz);
    }
    let mut p = project.clone();
    p.antenna.sources = s.bands[0].sources.iter().take(4).cloned().collect();
    // a frequency variable sets band 1's frequency
    let frequency_variable = s.variables.iter().any(|v| v.target == VariableTarget::Frequency);
    (p, if frequency_variable { frequency_mhz } else { s.bands[0].frequency_mhz })
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn an_optimisation_keeps_its_best_and_lists_what_it_calculated(case in case()) {
        // (the case as JSON, to replay it with `optimize` elsewhere)
        let json = serde_json::json!({ "project": case.project, "options": case.options, "setup": case.setup });
        keeps_its_best(&case).map_err(|e| TestCaseError::fail(format!("{e}\ncase {json}")))?;
    }
}

fn keeps_its_best(case: &Case) -> Result<(), TestCaseError> {
    {
        let c = case.clone();
        let (outcome, log) = within(&case.project, move || {
            let mut log = Vec::new();
            let out = optimize::optimize(&c.project, &c.options, &c.setup, Some(CALCULATIONS), |p| {
                log.push(p.clone());
                true
            });
            (out, log)
        });
        let Ok(out) = outcome else { return Ok(()) };
        prop_assert!(out.calculations <= CALCULATIONS);
        prop_assert_eq!(out.calculations, log.len());
        // the scores of the trials, in order (a trial's score comes with its last band)
        let scores: Vec<f64> = log.iter().filter_map(|p| p.score).collect();
        prop_assert!(scores.iter().all(|s| s.is_finite()), "scores {scores:?}");
        let Some(best) = &out.best else {
            // no trial finished, or every one took no power: the antenna
            // stays as it was
            prop_assert!(scores.iter().all(|&s| s == f64::MIN), "scores {scores:?} but no best");
            prop_assert_eq!(&out.project, &case.project);
            return Ok(());
        };
        // the best is the best of every trial, so no worse than the start
        let top = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        prop_assert_eq!(best.score, top);
        prop_assert!(best.score >= scores[0], "best {} below the start {}", best.score, scores[0]);
        // and what an improvement flagged
        let improved: Vec<f64> = log.iter().filter(|p| p.improved).filter_map(|p| p.score).collect();
        prop_assert_eq!(improved.last().copied(), Some(best.score));
        prop_assert!(improved.windows(2).all(|w| w[0] < w[1]), "{improved:?}");

        // the antenna the run ends with is the best trial written back (the
        // run sets the values on the antenna it varied, so to rounding), and
        // it calculates to the best trial's figures
        let (written, frequency) = optimize::apply_trial(&case.project, &case.options, &case.setup, &best.values)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(same_to_rounding(&written, &out.project), "{written:?}\n{:?}", out.project);
        prop_assert_eq!(frequency, out.frequency_mhz);
        let (p, f) = band_one(case, &out.project, out.frequency_mhz);
        let (got, takes_power) = Figures::calculated(&p, &case.options, f)
            .map_err(|e| TestCaseError::fail(format!("the optimised antenna does not calculate: {e}")))?;
        prop_assert!(
            got.close(&Figures::of_trial(best)),
            "optimised antenna: {got:?}, best {:?}",
            Figures::of_trial(best)
        );
        // a breakdown of the model is never the best
        prop_assert!(takes_power || case.setup.sweep_only, "the optimised antenna takes no power: {got:?}");

        // every trial listed is what its values give
        let mut listed: Vec<&Trial> = out.trials.iter().collect();
        listed.push(best);
        for t in listed {
            let (p, f) = optimize::apply_trial(&case.project, &case.options, &case.setup, &t.values)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            let (p, f) = band_one(case, &p, f);
            let want = Figures::of_trial(t);
            let (got, takes_power) = Figures::calculated(&p, &case.options, f).map_err(|e| {
                TestCaseError::fail(format!("trial {:?} written back does not calculate: {e}", t.values))
            })?;
            // (an antenna that takes no power has figures of rounding; such
            // a trial scored the least)
            if takes_power {
                prop_assert!(got.close(&want), "trial {:?} written back: {got:?}, listed {want:?}", t.values);
            } else if !case.setup.sweep_only {
                prop_assert_eq!(t.score, f64::MIN);
            }
        }
        // and the figures each trial was scored on
        for p in &log {
            if p.band == 0
                && let Some(t) = out.trials.iter().find(|t| t.values == p.values)
            {
                prop_assert_eq!(Figures::of_progress(p), Figures::of_trial(t), "calculation {}", p.calculation);
            }
        }
    }
    Ok(())
}
