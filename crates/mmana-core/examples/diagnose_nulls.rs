//! Read-only audit: emits every pattern / far-field mismatch as JSON Lines.
use mmana_core::{CalcOptions, GroundKind, calculate, maa, solve};
use serde_json::{Value, json};
use std::path::Path;

fn number(v: &Value) -> f64 {
    v.as_f64().expect("numeric golden value")
}

fn as_f32(v: &Value) -> f64 {
    number(v) as f32 as f64
}

// Invert the original engine's 4.343 * ln(power) convention.
// -999 is a sentinel for power <= 1e-30, not an ordinary dB measurement.
fn power(db: f64) -> f64 {
    if db == -999.0 { 0.0 } else { (db / 4.343).exp() }
}

fn compare(
    case: &str,
    kind: &str,
    polarization: &str,
    zenith: f64,
    azimuth: f64,
    actual: f64,
    expected: f64,
    tolerance: f64,
    peak: f64,
) -> bool {
    let delta = (actual - expected).abs();
    if actual.is_finite() && expected.is_finite() && delta <= tolerance {
        return false;
    }
    println!(
        "{}",
        json!({
            "record": "difference",
            "case": case, "kind": kind, "polarization": polarization,
            "zenithDeg": zenith, "azimuthDeg": azimuth,
            "actualDb": actual, "expectedDb": expected,
            "absoluteDbDifference": delta, "toleranceDb": tolerance,
            "actualSentinel": actual == -999.0, "expectedSentinel": expected == -999.0,
            "actualPower": power(actual), "expectedPower": power(expected),
            "absolutePowerDifference": (power(actual) - power(expected)).abs(),
            "peakTotalDb": peak,
            "actualPowerRelativeToPeak": power(actual) / power(peak),
            "expectedPowerRelativeToPeak": power(expected) / power(peak),
            "bothBelowMinus100Db": actual <= -100.0 && expected <= -100.0,
            "bothBelowMinus200Db": actual <= -200.0 && expected <= -200.0
        })
    );
    true
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<_> = std::fs::read_dir(root.join("testdata/golden"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    let mut differences = 0;
    let mut samples = 0;
    let mut cases = 0;
    let mut far_cases = 0;
    println!("{}", json!({"record":"environment","os":std::env::consts::OS,"arch":std::env::consts::ARCH}));
    for path in paths {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let (stem, variant) = name.split_once('@').unwrap_or((name, ""));
        let g: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let mut project =
            maa::parse_bytes(&std::fs::read(root.join(format!("testdata/maa/{stem}.MAA"))).unwrap()).project;
        if let Some(v) = variant.strip_prefix("ground") {
            project.environment.ground = GroundKind::from_code(v[..1].parse().unwrap());
        } else if let Some(v) = variant.strip_prefix('h') {
            project.environment.ground = GroundKind::Real;
            project.environment.height = v.parse().unwrap();
        }
        let opts = CalcOptions { frequency_mhz: Some(number(&g["freq"])), max_pulses: 4096, ..Default::default() };
        let r = calculate(&project, &opts).unwrap();
        let peak = (r.gain_dbi as f64).max(as_f32(&g["result"]["gain"]));
        let before = differences;
        for (kind, cut, key) in [("horizontal", &r.patterns.horizontal, "h"), ("vertical", &r.patterns.vertical, "v")] {
            for (pol, (label, values)) in
                [("vertical", &cut.vertical_pol), ("horizontal", &cut.horizontal_pol), ("total", &cut.total)]
                    .into_iter()
                    .enumerate()
            {
                let expected = g["pattern"][key][pol].as_array().unwrap();
                assert_eq!(values.len(), expected.len());
                for (i, (&a, b)) in values.iter().zip(expected).enumerate() {
                    let angle = cut.start_deg + i as f64 * cut.step_deg;
                    let (zenith, azimuth) = if kind == "horizontal" {
                        (90.0 - r.patterns.horizontal_elevation_deg as f64, angle)
                    } else if angle < 90.0 {
                        (90.0 - angle, 180.0)
                    } else {
                        (angle - 90.0, 0.0)
                    };
                    samples += 1;
                    differences +=
                        compare(name, kind, label, zenith, azimuth, a as f64, as_f32(b), 1e-3, peak) as usize;
                }
            }
        }
        if let Some(far) = g.get("far") {
            far_cases += 1;
            let s = solve(&project, &opts).unwrap();
            let actual = s.far_field((0.0, 15.0, 13), (0.0, 45.0, 8));
            let expected = far.as_array().unwrap();
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(expected) {
                assert!((a.zenith - number(&b[0])).abs() <= 5.0001e-4);
                assert!((a.azimuth - number(&b[1])).abs() <= 5.0001e-4);
                for (label, value, index) in
                    [("vertical", a.vertical_db, 2), ("horizontal", a.horizontal_db, 3), ("total", a.total_db, 4)]
                {
                    samples += 1;
                    differences +=
                        compare(name, "far", label, a.zenith, a.azimuth, value, number(&b[index]), 5.0001e-4, peak)
                            as usize;
                }
            }
        }
        cases += 1;
        println!("{}", json!({"record":"case","case":name,"differences":differences-before}));
    }
    assert_eq!(cases, 149);
    assert_eq!(far_cases, 8);
    println!(
        "{}",
        json!({"record":"summary","cases":cases,"farCases":far_cases,"samples":samples,"differences":differences})
    );
}
