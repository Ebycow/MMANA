//! Compares the Rust engine with the original C++ MININEC3 engine.
//!
//! `testdata/golden/*.json` is produced by `reference/oracle` (the unmodified
//! mininec3.cpp compiled with clang) for every sample in `testdata/maa` and
//! model in `testdata/models`, plus ground / height / frequency variants.
//! File names are `STEM[@variant].json`.

use mmana_core::model::{GroundScreen, Medium};
use mmana_core::{AxisRange, CalcOptions, GroundKind, NearFieldKind, NearFieldSpec, Project, calculate, maa, solve};
use serde_json::Value;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// Values the original stores as `float`, printed with `%.9g`.
fn f32v(v: &Value) -> f64 {
    f(v) as f32 as f64
}

/// Difference of one sample. A non-finite value never matches (`f64::max`
/// would otherwise drop a NaN; the engine's "no power" sentinel is the finite
/// -999), and samples where both sides are at or below `null_floor` count as
/// equal nulls.
fn sample_diff(x: f64, y: f64, null_floor: f64) -> f64 {
    if !x.is_finite() || !y.is_finite() {
        f64::INFINITY
    } else if x == y || (x <= null_floor && y <= null_floor) {
        0.0
    } else {
        (x - y).abs()
    }
}

/// Largest absolute difference between two float series.
fn max_diff(a: impl Iterator<Item = f64>, b: impl Iterator<Item = f64>) -> f64 {
    max_db_diff(a, b, f64::NEG_INFINITY)
}

/// Largest dB difference, ignoring samples where both sides are nulls.
fn max_db_diff(a: impl Iterator<Item = f64>, b: impl Iterator<Item = f64>, null_floor: f64) -> f64 {
    a.zip(b).map(|(x, y)| sample_diff(x, y, null_floor)).fold(0.0, f64::max)
}

fn worst_case_figures_exact(r: &mmana_core::CalcResult, g: &Value) -> bool {
    let res = &g["result"];
    [
        (r.gain_dbi, &res["gain"]),
        (r.fb_db, &res["fb"]),
        (r.elevation_deg, &res["elv"]),
        (r.max_horizontal_pol_dbi, &res["maxhp"]),
        (r.max_vertical_pol_dbi, &res["maxvp"]),
    ]
    .iter()
    .all(|(a, b)| *a as f64 == f32v(b))
}

/// Loads the sample for `STEM[@variant]` and applies ground/height variants,
/// and the real ground the oracle was given (not part of a `.maa` file).
fn load_project(name: &str, golden: &Value) -> Project {
    let stem = name.split('@').next().unwrap();
    // a shipped sample, or a model for an engine path no sample takes
    let path = ["maa", "models"].map(|dir| root().join(format!("testdata/{dir}/{stem}.MAA")));
    let bytes = std::fs::read(path.iter().find(|p| p.exists()).unwrap()).unwrap();
    let mut project = maa::parse_bytes(&bytes).project;
    if let Some(variant) = name.split('@').nth(1) {
        if let Some(gr) = variant.strip_prefix("ground") {
            project.environment.ground = GroundKind::from_code(gr[..1].parse().unwrap());
        } else if let Some(h) = variant.strip_prefix('h') {
            project.environment.ground = GroundKind::Real;
            project.environment.height = h.parse().unwrap();
        }
    }
    if let Some(rg) = golden.get("real_ground") {
        let real_ground = &mut project.environment.real_ground;
        real_ground.media = rg["media"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| Medium {
                permittivity: f(&m[0]),
                conductivity: f(&m[1]),
                boundary: m[2].as_f64(),
                height: m[3].as_f64(),
            })
            .collect();
        real_ground.screen = rg["screen"]
            .as_array()
            .map(|s| GroundScreen { radials: s[0].as_i64().unwrap() as i32, radial_radius: f(&s[1]) });
    }
    project
}

struct Case {
    name: String,
    golden: Value,
}

fn cases() -> Vec<Case> {
    let mut v: Vec<Case> = std::fs::read_dir(root().join("testdata/golden"))
        .expect("testdata/golden (run reference/oracle/gen_golden.sh)")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| Case {
            name: p.file_stem().unwrap().to_string_lossy().into_owned(),
            golden: serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap(),
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// The goldens were generated on macOS/arm64. The port reproduces the
/// original's arithmetic, so there every value must be bit-identical.
const GOLDEN_PLATFORM: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

/// Elsewhere a different libm can change `sin`/`log` by 1 ulp, which the
/// `f32` impedance matrix amplifies; these margins are far below anything
/// visible in MMANA (impedance shown to 0.01 Ω, gain to 0.01 dB).
const TOL_CURRENT_REL: f64 = 1e-5;
const TOL_DB: f64 = 1e-3;
const TOL_OHM_REL: f64 = 1e-5;

/// Verification scope for pattern / far-field samples: components deeper than
/// this below the peak gain are not checked when both sides are that deep.
///
/// The far field is summed in `f64`, so a 1-ulp libm difference moves each
/// sample's field by about 1e-16·k of the peak field (k ≈ a few in the
/// observed cases). At peak−200 dB the field is 1e-10 of the peak, so that
/// noise is ~1e-6·k relative, ≈ 1e-5·k dB: everything above the floor is
/// still held to `TOL_DB` with ~100× margin. Below it, values are rounding
/// residue of an exact cancellation (seen at peak−270…−310 dB at the horizon
/// over ground, or the -999 sentinel) that shifts by tenths of a dB or more
/// between libms. The `f32` impedance matrix already limits physical accuracy
/// to roughly peak−140 dB, so nothing meaningful is lost. It applies to every
/// direction and polarisation, since exact cancellation (ground at the
/// horizon, symmetry nulls, cross-polar of planar antennas) can occur
/// anywhere. On the golden platform the raw values are still required to be
/// bit-identical.
const NULL_BELOW_PEAK_DB: f64 = 200.0;

#[test]
fn engine_matches_original() {
    let cases = cases();
    assert!(cases.len() > 100, "expected the full golden set, found {}", cases.len());
    let mut failures = Vec::new();
    let mut worst = (0.0f64, 0.0f64, 0.0f64);
    let mut exact = 0;

    for case in &cases {
        let project = load_project(&case.name, &case.golden);
        let g = &case.golden;
        let opts = CalcOptions { frequency_mhz: Some(f(&g["freq"])), max_pulses: 4096, ..Default::default() };
        let r = match calculate(&project, &opts) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{}: {e}", case.name));
                continue;
            }
        };
        let mut problems = Vec::new();

        if r.pulse_count as u64 != g["pulses"].as_u64().unwrap() {
            problems.push(format!("pulses {} != {}", r.pulse_count, g["pulses"]));
        }
        let gwz = g["wz"].as_array().unwrap();
        if r.segmented_wires.len() != gwz.len() {
            problems.push(format!("segment wires {} != {}", r.segmented_wires.len(), gwz.len()));
        } else {
            for (w, gw) in r.segmented_wires.iter().zip(gwz) {
                let gw = gw.as_array().unwrap();
                let ours = [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z, w.radius];
                if ours.iter().zip(gw).any(|(a, b)| *a != f(b)) || w.segments as i64 != gw[7].as_i64().unwrap() {
                    problems.push(format!("segment wire differs: {ours:?} seg {} vs {gw:?}", w.segments));
                    break;
                }
            }
        }
        let gsrc: Vec<u64> = g["sources"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        let osrc: Vec<u64> = r.sources.iter().map(|s| s.pulse as u64).collect();
        if gsrc != osrc {
            problems.push(format!("source pulses {osrc:?} != {gsrc:?}"));
        }

        // currents
        let gcur = g["currents"].as_array().unwrap();
        let scale = gcur.iter().map(|c| f(&c[0]).hypot(f(&c[1]))).fold(0.0, f64::max);
        let dc = max_diff(
            r.currents.iter().flat_map(|c| [c.current.re, c.current.im]),
            gcur.iter().flat_map(|c| [f(&c[0]), f(&c[1])]),
        ) / scale;
        worst.0 = worst.0.max(dc);
        if dc > TOL_CURRENT_REL {
            problems.push(format!("currents rel diff {dc:e}"));
        }

        // headline figures (stored as f32 by the original)
        let res = &g["result"];
        let rel = |a: f64, b: f64| if a == b { 0.0 } else { (a - b).abs() / b.abs().max(1.0) };
        let dz = rel(r.impedance.re as f32 as f64, f32v(&res["r"]))
            .max(rel(r.impedance.im as f32 as f64, f32v(&res["jx"])))
            .max(rel(r.swr, f32v(&res["swr"])));
        worst.1 = worst.1.max(dz);
        if dz > TOL_OHM_REL {
            problems.push(format!("impedance/SWR rel diff {dz:e}"));
        }
        let figures = [
            ("gain", r.gain_dbi, &res["gain"]),
            ("fb", r.fb_db, &res["fb"]),
            ("elv", r.elevation_deg, &res["elv"]),
            ("maxhp", r.max_horizontal_pol_dbi, &res["maxhp"]),
            ("maxvp", r.max_vertical_pol_dbi, &res["maxvp"]),
            ("elv_h", r.patterns.horizontal_elevation_deg, &g["pattern"]["elv_h"]),
        ];
        for (name, ours, theirs) in figures {
            let d = (ours as f64 - f32v(theirs)).abs();
            worst.2 = worst.2.max(d);
            if d > TOL_DB {
                problems.push(format!("{name} {ours} != {theirs}"));
            }
        }

        // pattern cuts: `dp` is the raw difference, `dp_tol` ignores nulls
        let null_floor = f32v(&res["gain"]) - NULL_BELOW_PEAK_DB;
        let cut = |ours: [&Vec<f32>; 3], theirs: &Value, floor: f64| {
            (0..3)
                .map(|k| {
                    let t = theirs[k].as_array().unwrap();
                    max_db_diff(ours[k].iter().map(|&v| v as f64), t.iter().take(ours[k].len()).map(f32v), floor)
                })
                .fold(0.0, f64::max)
        };
        let h = &r.patterns.horizontal;
        let v = &r.patterns.vertical;
        let cuts = |floor: f64| {
            cut([&h.vertical_pol, &h.horizontal_pol, &h.total], &g["pattern"]["h"], floor).max(cut(
                [&v.vertical_pol, &v.horizontal_pol, &v.total],
                &g["pattern"]["v"],
                floor,
            ))
        };
        let dp = cuts(f64::NEG_INFINITY);
        let dp_tol = cuts(null_floor);
        worst.2 = worst.2.max(dp_tol);
        if dp_tol > TOL_DB {
            problems.push(format!("pattern diff {dp_tol} dB"));
        }

        // displayed current distribution
        let gdisp: Vec<f64> = g["cur"].as_array().unwrap().iter().map(f32v).collect();
        if gdisp.len() != r.current_display.len() {
            problems.push(format!("current display length {} != {}", r.current_display.len(), gdisp.len()));
        } else {
            let dd = max_diff(r.current_display.iter().map(|&v| v as f64), gdisp.iter().copied()) / scale;
            if dd > 1e-6 {
                problems.push(format!("current display rel diff {dd:e}"));
            }
        }

        if dc == 0.0 && dz == 0.0 && dp == 0.0 && worst_case_figures_exact(&r, g) {
            exact += 1;
        }
        if !problems.is_empty() {
            failures.push(format!("{}: {}", case.name, problems.join("; ")));
        }
    }

    println!(
        "{} cases, {} bit-identical; worst: currents rel {:e}, impedance rel {:e}, dB {:e}",
        cases.len(),
        exact,
        worst.0,
        worst.1,
        worst.2
    );
    assert!(failures.is_empty(), "{} case(s) differ:\n{}", failures.len(), failures.join("\n"));
    if GOLDEN_PLATFORM {
        assert_eq!(exact, cases.len(), "on the golden platform every case must be bit-identical");
    }
}

/// Near-field and far-field exports. The original writes these with
/// `%le` / `%.3lf`, so the comparison is limited to the printed precision.
#[test]
fn exports_match_original() {
    let mut checked = 0;
    for case in cases() {
        let g = &case.golden;
        if g.get("near").is_none() && g.get("far").is_none() {
            continue;
        }
        let project = load_project(&case.name, &case.golden);
        let opts = CalcOptions { frequency_mhz: Some(f(&g["freq"])), max_pulses: 4096, ..Default::default() };
        let solved = solve(&project, &opts).unwrap();

        if let Some(near) = g.get("near") {
            let spec = &g["near_spec"];
            let axis = |v: &Value| AxisRange { start: f(&v[0]), step: f(&v[1]), count: v[2].as_i64().unwrap() as i32 };
            let ours = solved
                .near_field(&NearFieldSpec {
                    kind: if spec["kind"] == "H" { NearFieldKind::Magnetic } else { NearFieldKind::Electric },
                    power_w: f(&spec["power"]),
                    x: axis(&spec["x"]),
                    y: axis(&spec["y"]),
                    z: axis(&spec["z"]),
                })
                .unwrap();
            let theirs = near.as_array().unwrap();
            assert_eq!(ours.len(), theirs.len(), "{}: near-field sample count", case.name);
            let scale = theirs.iter().map(|t| f(&t[6])).fold(0.0, f64::max);
            for (o, t) in ours.iter().zip(theirs) {
                assert_eq!(o.component.to_string(), t[3].as_str().unwrap(), "{}", case.name);
                for (a, b) in [(o.real, &t[4]), (o.imag, &t[5]), (o.magnitude, &t[6])] {
                    // %le keeps 7 significant digits
                    let tol = f(b).abs() * 1e-6 + scale * 1e-12;
                    assert!((a - f(b)).abs() <= tol, "{}: near {a} vs {b}", case.name);
                }
                // the phase lies within ±180°
                assert!(o.phase_deg.abs() <= 180.0, "{}: phase {}", case.name, o.phase_deg);
                if f(&t[6]) > scale * 1e-6 {
                    let d = (o.phase_deg - f(&t[7])).abs();
                    assert!(d <= 1e-5 || (d - 360.0).abs() <= 1e-5, "{}: phase {} vs {}", case.name, o.phase_deg, t[7]);
                }
            }
            checked += 1;
        }
        if let Some(far) = g.get("far") {
            let ours = solved.far_field((0.0, 15.0, 13), (0.0, 45.0, 8)).unwrap();
            let theirs = far.as_array().unwrap();
            assert_eq!(ours.len(), theirs.len(), "{}: far-field sample count", case.name);
            let null_floor = f32v(&g["result"]["gain"]) - NULL_BELOW_PEAK_DB;
            let grounded = project.environment.ground != GroundKind::FreeSpace;
            let mut below = 0;
            for (o, t) in ours.iter().zip(theirs) {
                let vals = [o.zenith, o.azimuth, o.vertical_db, o.horizontal_db, o.total_db];
                // below a ground's horizon the original gives the image's field;
                // the port gives none (porting-notes.md)
                if grounded && (o.zenith * PI / 180.0).cos() < -1e-12 {
                    assert_eq!(&vals[..2], [f(&t[0]), f(&t[1])], "{}", case.name);
                    assert_eq!(&vals[2..], [-999.0; 3], "{}: far {vals:?}", case.name);
                    below += 1;
                    continue;
                }
                for (k, (&a, b)) in vals.iter().zip(t.as_array().unwrap()).enumerate() {
                    // angles are compared in full; only the dB columns may be nulls
                    let floor = if k >= 2 { null_floor } else { f64::NEG_INFINITY };
                    assert!(sample_diff(a, f(b), floor) <= 5.0001e-4, "{}: far {vals:?} vs {t}", case.name);
                }
            }
            // zenith 0 to 180 in steps of 15: 6 of the 13 are below
            assert_eq!(below, if grounded { 6 * 8 } else { 0 }, "{}", case.name);
            checked += 1;
        }
    }
    assert!(checked >= 16, "expected near/far goldens, checked {checked}");
}

#[test]
fn sample_comparison_handles_nulls_and_non_finite_values() {
    let floor = 12.0 - NULL_BELOW_PEAK_DB; // -188 dB
    // non-finite values never match, not even as nulls or with themselves
    assert_eq!(sample_diff(f64::NAN, -300.0, floor), f64::INFINITY);
    assert_eq!(sample_diff(-300.0, f64::NAN, floor), f64::INFINITY);
    assert_eq!(sample_diff(f64::NAN, f64::NAN, floor), f64::INFINITY);
    assert_eq!(sample_diff(f64::NEG_INFINITY, -300.0, floor), f64::INFINITY);
    assert_eq!(sample_diff(f64::NEG_INFINITY, f64::NEG_INFINITY, floor), f64::INFINITY);
    assert_eq!(sample_diff(f64::INFINITY, -10.0, floor), f64::INFINITY);
    assert_eq!(max_db_diff([f64::NAN, -10.0].into_iter(), [-300.0, -10.0].into_iter(), floor), f64::INFINITY);
    assert_eq!(max_diff([f64::NAN].into_iter(), [1.0].into_iter()), f64::INFINITY);
    // the finite -999 sentinel and residue below the floor are equal nulls
    assert_eq!(sample_diff(-999.0, -299.9, floor), 0.0);
    assert_eq!(sample_diff(-290.9, -291.7, floor), 0.0);
    // the floor itself is a null; anything above it is compared
    assert_eq!(sample_diff(floor, -999.0, floor), 0.0);
    assert!(sample_diff(floor + 0.01, -999.0, floor) > 800.0);
    assert_eq!(sample_diff(-10.0, -300.0, floor), 290.0);
    // without a floor every difference counts
    assert_eq!(sample_diff(-999.0, -300.0, f64::NEG_INFINITY), 699.0);
}
