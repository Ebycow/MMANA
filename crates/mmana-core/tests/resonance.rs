//! The resonance search and the series circuit fitted near it.

use mmana_core::resonance::{
    MAX_RESONANCE_CALCULATIONS, RESONANCE_REACTANCE, ResonanceRange, ResonanceSearch, series_circuit,
};
use mmana_core::{CalcOptions, Complex, Point3, Project, Segmentation, Source, Wire, maa, solve};
use std::f64::consts::PI;
use std::path::Path;

fn dipole(half: f64) -> Project {
    let mut p = Project::default();
    p.antenna.frequency_mhz = 14.05;
    p.antenna.wires = vec![Wire {
        start: Point3::new(0.0, -half, 0.0),
        end: Point3::new(0.0, half, 0.0),
        radius: 0.001,
        segmentation: Segmentation::Fixed { count: 21 },
        note: String::new(),
        group: String::new(),
    }];
    p.antenna.sources = vec![Source { pulse: "w1c".into(), phase_deg: 0.0, voltage: 1.0 }];
    p
}

fn search(p: &Project, start: Option<f64>) -> ResonanceSearch {
    let range = ResonanceRange { start_mhz: start, ..Default::default() };
    mmana_core::resonance::find_resonance(p, &CalcOptions::default(), &range).unwrap()
}

fn reactance(p: &Project, mhz: f64) -> f64 {
    solve(p, &CalcOptions { frequency_mhz: Some(mhz), ..Default::default() }).unwrap().feed().0.im
}

#[test]
fn a_series_circuit_is_recovered_from_two_frequencies() {
    let (r, l, c) = (50.0, 10e-6, 12.8e-12);
    let z = |mhz: f64| {
        let w = 2.0 * PI * mhz * 1e6;
        (mhz, Complex { re: r, im: w * l - 1.0 / (w * c) })
    };
    let fit = series_circuit(z(14.0), z(14.3)).unwrap();
    let f0 = 1.0 / (2.0 * PI * (l * c).sqrt()) / 1e6;
    let q = (l / c).sqrt() / r;
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs();
    assert!(close(fit.inductance_uh, 10.0), "{fit:?}");
    assert!(close(fit.capacitance_pf, 12.8), "{fit:?}");
    assert!(close(fit.resonance_mhz, f0), "{fit:?}");
    assert!(close(fit.q, q), "{fit:?}");
    assert!(close(fit.bandwidth_khz, f0 * 1e3 / q), "{fit:?}");
    // the same frequency twice, or a reactance falling with frequency, is no series circuit
    assert_eq!(series_circuit(z(14.0), z(14.0)), None);
    let falling = |mhz: f64| (mhz, Complex { re: r, im: -z(mhz).1.im });
    assert_eq!(series_circuit(falling(14.0), falling(14.3)), None);
    // nor is one without a resistance (its Q would be infinite)
    let lossless = |mhz: f64| (mhz, Complex { re: 0.0, im: z(mhz).1.im });
    assert_eq!(series_circuit(lossless(14.0), lossless(14.3)), None);
}

#[test]
fn finds_where_the_reactance_of_a_dipole_crosses_zero() {
    let p = dipole(5.0);
    let found = search(&p, None);
    let r = found.resonance.expect("a half-wave dipole resonates");
    assert!(r.converged && r.impedance.im.abs() < RESONANCE_REACTANCE, "{r:?}");
    // X = 0 lies within 0.01 % of it (the original stops within 0.1 %)
    let f = r.frequency_mhz;
    assert!(reactance(&p, f * (1.0 - 1e-4)) < 0.0 && reactance(&p, f * (1.0 + 1e-4)) > 0.0, "{r:?}");
    assert!((60.0..80.0).contains(&r.impedance.re), "{r:?}");
    // a dipole of Q around 10
    let c = r.circuit.expect("fitted near resonance");
    assert!((5.0..20.0).contains(&c.q), "{c:?}");
    assert!((c.resonance_mhz / f - 1.0).abs() < 1e-3, "{c:?}");
    // the fitted circuit's guesses take it there in 4 (halving alone, or
    // stepping away from them, takes twice as many)
    assert!(found.calculations <= 6, "{} calculations", found.calculations);
}

#[test]
fn finds_the_same_resonance_from_either_side() {
    let p = dipole(5.0);
    let at = |start| search(&p, Some(start)).resonance.unwrap().frequency_mhz;
    let (below, above) = (at(12.0), at(16.0));
    assert!((below / above - 1.0).abs() < 1e-4, "{below} vs {above}");
}

#[test]
fn reports_no_resonance_when_there_is_none_in_range() {
    // a 2 m dipole resonates near 70 MHz, beyond twice 14.05 MHz
    let found = search(&dipole(1.0), None);
    assert_eq!(found.resonance, None);
    assert_eq!((found.min_mhz, found.max_mhz), (14.05 / 2.0, 14.05 * 2.0));
    assert!(found.calculations <= MAX_RESONANCE_CALCULATIONS);
    // a range reaching it finds it
    let range = ResonanceRange { max_mhz: Some(100.0), ..Default::default() };
    let r = mmana_core::resonance::find_resonance(&dipole(1.0), &CalcOptions::default(), &range).unwrap();
    assert!(r.resonance.as_ref().is_some_and(|r| r.converged && (50.0..90.0).contains(&r.frequency_mhz)), "{r:?}");
}

#[test]
fn rejects_a_start_outside_the_range() {
    let range = ResonanceRange { start_mhz: Some(30.0), max_mhz: Some(20.0), ..Default::default() };
    assert!(mmana_core::resonance::find_resonance(&dipole(5.0), &CalcOptions::default(), &range).is_err());
    // nor can a range reach down to 0 MHz
    let range = ResonanceRange { min_mhz: Some(0.0), ..Default::default() };
    assert!(mmana_core::resonance::find_resonance(&dipole(5.0), &CalcOptions::default(), &range).is_err());
}

/// Every sample: a resonance found has |X| under 0.1 Ω, or X jumps across 0
/// at it; the search keeps to its limit of calculations.
#[test]
fn every_sample_ends_at_a_resonance_or_at_none() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa");
    let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).collect();
    names.sort();
    let (mut converged, mut jumps, mut none) = (0, 0, 0);
    for path in names {
        let p = maa::parse_bytes(&std::fs::read(&path).unwrap()).project;
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // the heavy ones take long and add nothing here
        if mmana_core::pulse_layout(&p, &CalcOptions::default()).map_or(true, |l| l.pulse_count > 600) {
            continue;
        }
        let Ok(found) = mmana_core::resonance::find_resonance(&p, &CalcOptions::default(), &ResonanceRange::default())
        else {
            continue;
        };
        assert!(found.calculations <= MAX_RESONANCE_CALCULATIONS, "{name}");
        match found.resonance {
            None => none += 1,
            Some(r) if r.converged => {
                assert!(r.impedance.im.abs() < RESONANCE_REACTANCE, "{name}: {r:?}");
                converged += 1;
            }
            Some(r) => {
                // the reactance changes sign within a hair of it
                let f = r.frequency_mhz;
                let (a, b) = (reactance(&p, f * (1.0 - 1e-7)), reactance(&p, f * (1.0 + 1e-7)));
                assert!(
                    a < 0.0 && b > 0.0 || a > 0.0 && b < 0.0 || a.abs().min(b.abs()) < 1.0,
                    "{name}: {r:?} ({a}, {b})"
                );
                jumps += 1;
            }
        }
    }
    eprintln!("converged {converged}, jumps {jumps}, none {none}");
    assert!(converged > 30, "converged {converged}, jumps {jumps}, none {none}");
}
