//! The calculators against the original's (`testdata/circuits/golden.json`,
//! from `reference/oracle/tools_main.cpp` on `testdata/circuits/cases.txt`):
//! every value bit for bit.

use mmana_core::Complex;
use mmana_core::circuits::*;
use serde_json::Value;

/// `CorePT` / `CoreZT` of OptDlg.cpp: the velocity factors are `float`s.
const CABLES: [(f32, f32); 20] = [
    (0.98, 600.0),
    (0.98, 600.0),
    (0.98, 450.0),
    (0.85, 300.0),
    (0.67, 75.0),
    (0.67, 75.0),
    (0.67, 75.0),
    (0.67, 75.0),
    (0.67, 50.0),
    (0.67, 50.0),
    (0.67, 53.5),
    (0.67, 50.0),
    (0.81, 50.0),
    (0.81, 50.0),
    (0.80, 50.0),
    (0.81, 50.0),
    (0.83, 50.0),
    (0.83, 50.0),
    (0.88, 50.0),
    (0.88, 50.0),
];

struct Check<'a> {
    case: &'a str,
    golden: &'a Value,
    failures: Vec<String>,
}

impl Check<'_> {
    fn num(&mut self, key: &str, got: Option<f64>) {
        self.at(self.golden, key, got);
    }

    fn at(&mut self, obj: &Value, key: &str, got: Option<f64>) {
        let want = obj.get(key).and_then(Value::as_f64);
        let same = match (want, got) {
            (Some(w), Some(g)) => w.to_bits() == g.to_bits(),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.failures.push(format!("{}: {key}: original {want:?}, port {got:?}", self.case));
        }
    }

    fn kind(&mut self, obj: &Value, key: &str, got: ElementKind) {
        let want = obj[key].as_str().unwrap_or_default();
        let got = match got {
            ElementKind::L => "L",
            ElementKind::C => "C",
        };
        if want != got {
            self.failures.push(format!("{}: {key}: original {want}, port {got}", self.case));
        }
    }

    fn fail(&mut self, what: impl std::fmt::Display) {
        self.failures.push(format!("{}: {what}", self.case));
    }
}

fn arg(a: &[&str], i: usize) -> f64 {
    a[i].parse().unwrap_or_else(|_| panic!("bad number {:?}", a[i]))
}

fn z(re: f64, im: f64) -> Complex {
    Complex { re, im }
}

fn check(c: &mut Check, a: &[&str]) {
    match a[0] {
        "tank" => {
            let (p, q) = (arg(a, 2), arg(a, 3));
            let input = match a[1] {
                "x" => TankInput::Reactance { frequency_mhz: p, reactance_ohm: q },
                "l" => TankInput::Inductance { frequency_mhz: p, inductance_uh: q },
                "c" => TankInput::Capacitance { frequency_mhz: p, capacitance_pf: q },
                _ => TankInput::Resonance { inductance_uh: p, capacitance_pf: q },
            };
            match tank(&input) {
                Ok(t) => {
                    c.num("freq", t.frequency_mhz);
                    c.num("x", t.reactance_ohm);
                    c.num("l", t.inductance_uh);
                    c.num("c", t.capacitance_pf);
                    c.num("wl", t.wavelength.map(|w| w.full_m));
                    c.num("wl2", t.wavelength.map(|w| w.half_m));
                    c.num("wl4", t.wavelength.map(|w| w.quarter_m));
                    c.num("wl58", t.wavelength.map(|w| w.five_eighths_m));
                }
                // the dialog refuses L or C of 0 (TEditLChange, TEditCChange)
                Err(e) if q == 0.0 || p == 0.0 => drop(e),
                Err(e) => c.fail(e),
            }
        }
        "coil" => {
            let (p, q) = (arg(a, 2), arg(a, 3));
            let unknown = match a[1] {
                "l" => CoilUnknown::Inductance { turns: p, diameter_cm: q },
                "n" => CoilUnknown::Turns { inductance_uh: p, diameter_cm: q },
                _ => CoilUnknown::Diameter { inductance_uh: p, turns: q },
            };
            match coil(&CoilInput { unknown, wire_diameter_mm: arg(a, 4), spacing_mm: arg(a, 5) }) {
                Ok(r) => {
                    c.num("l", Some(r.inductance_uh));
                    c.num("n", Some(r.turns));
                    c.num("d", Some(r.diameter_cm));
                    c.num("len", Some(r.length_cm));
                }
                Err(e) => c.fail(e),
            }
        }
        "match" => {
            let circuit = if a[5] == "1" { LcCircuit::SeriesC } else { LcCircuit::ShuntC };
            let input = LcMatchInput {
                frequency_mhz: arg(a, 1),
                line_ohm: arg(a, 2),
                antenna: z(arg(a, 3), arg(a, 4)),
                circuit,
            };
            match lc_match(&input) {
                Ok(m) => {
                    let g = c.golden;
                    let mt = if m.shunt_side == ShuntSide::Antenna { 1 } else { 0 };
                    if (m.shunt.is_some() || m.series.is_some()) && g["mt"].as_i64() != Some(mt) {
                        c.fail(format!("mt: original {}, port {mt}", g["mt"]));
                    }
                    for (key, e) in [("shunt", m.shunt), ("series", m.series)] {
                        match (&g[key], e) {
                            (Value::Null, None) => {}
                            (want @ Value::Object(_), Some(e)) => {
                                c.kind(want, "kind", e.kind);
                                c.at(want, "value", Some(e.value));
                                c.at(want, "x", Some(e.reactance_ohm));
                            }
                            (want, got) => c.fail(format!("{key}: original {want}, port {got:?}")),
                        }
                    }
                }
                Err(e) => c.fail(e),
            }
        }
        "stub" => {
            let t: usize = a[2].parse().unwrap();
            let cable = if t == 0 {
                Cable { velocity_factor: arg(a, 3), impedance_ohm: arg(a, 4) }
            } else {
                Cable { velocity_factor: CABLES[t].0 as f64, impedance_ohm: CABLES[t].1 as f64 }
            };
            let end = if a[5] == "short" { StubEnd::Short } else { StubEnd::Open };
            let v = arg(a, 7);
            let given = match a[6] {
                "x" => StubGiven::Reactance { reactance_ohm: v },
                "l" => StubGiven::Inductance { inductance_uh: v },
                "c" => StubGiven::Capacitance { capacitance_pf: v },
                _ => StubGiven::Length { length_cm: v },
            };
            c.num("p", Some(cable.velocity_factor));
            c.num("z", Some(cable.impedance_ohm));
            match stub(&StubInput { frequency_mhz: arg(a, 1), cable, end, given }) {
                Ok(s) => {
                    // an open circuit is -9.999999e37 in the original
                    let open = c.golden["x"].as_f64() == Some(-9.999999e37);
                    if open != s.reactance_ohm.is_none() {
                        c.fail(format!("open: original {}, port {:?}", c.golden["x"], s.reactance_ohm));
                    }
                    c.num("x", Some(s.element.reactance_ohm));
                    c.kind(c.golden, "kind", s.element.kind);
                    c.num("lc", Some(s.element.value));
                    c.num("len", Some(s.length_cm));
                    c.num("fo", s.quarter_wave_mhz);
                }
                Err(e) => c.fail(e),
            }
        }
        "line" => {
            let known = z(arg(a, 5), arg(a, 6));
            let known =
                if a[1] == "zi" { LineEnd::Load { impedance: known } } else { LineEnd::Input { impedance: known } };
            match line(&LineInput { impedance_ohm: arg(a, 2), length_wl: arg(a, 3), loss_db: arg(a, 4), known }) {
                Ok(l) => {
                    c.num("ri", l.input.map(|z| z.re));
                    c.num("xi", l.input.map(|z| z.im));
                    c.num("rl", l.load.map(|z| z.re));
                    c.num("xl", l.load.map(|z| z.im));
                    c.num("swri", l.input_swr);
                    c.num("swrl", l.load_swr);
                }
                Err(e) => c.fail(e),
            }
        }
        "qm" | "qmtune" => {
            let r = if a[0] == "qm" {
                two_section(&TwoSectionInput {
                    load: z(arg(a, 1), arg(a, 2)),
                    reference_ohm: arg(a, 3),
                    feed_section: LineSection { impedance_ohm: arg(a, 4), length_wl: arg(a, 5) },
                    load_section: LineSection { impedance_ohm: arg(a, 6), length_wl: arg(a, 7) },
                })
            } else {
                tune_two_section(&TwoSectionTuneInput {
                    load: z(arg(a, 1), arg(a, 2)),
                    reference_ohm: arg(a, 3),
                    feed_impedance_ohm: arg(a, 4),
                    load_impedance_ohm: arg(a, 5),
                })
            };
            match r {
                Ok(t) => {
                    c.num("l1", Some(t.feed_section.length_wl));
                    c.num("l2", Some(t.load_section.length_wl));
                    c.num("rj", t.junction.map(|z| z.re));
                    c.num("xj", t.junction.map(|z| z.im));
                    c.num("ri", t.input.map(|z| z.re));
                    c.num("xi", t.input.map(|z| z.im));
                    c.num("swr", t.swr);
                }
                Err(e) => c.fail(e),
            }
        }
        "sm" => {
            let input = StubMatchInput {
                frequency_mhz: arg(a, 1),
                reference_ohm: arg(a, 2),
                load: z(arg(a, 3), arg(a, 4)),
                cable: Cable { impedance_ohm: arg(a, 5), velocity_factor: arg(a, 6) },
            };
            match stub_match(&input) {
                Ok(m) => {
                    let found = c.golden["found"].as_u64().unwrap() as usize;
                    if m.solutions.len() != found {
                        c.fail(format!("found: original {found}, port {}", m.solutions.len()));
                        return;
                    }
                    // the original keeps the first and the last
                    let want = c.golden["solutions"].as_array().unwrap().clone();
                    let got = [m.solutions.first(), m.solutions.last()];
                    for (w, s) in want.iter().zip(got) {
                        let s = s.unwrap();
                        c.at(w, "l", Some(s.position_wl));
                        c.at(w, "lcm", Some(s.position_cm));
                        c.at(w, "r", Some(s.impedance.re));
                        c.at(w, "x", Some(s.impedance.im));
                        c.at(w, "xc", s.stub.map(|t| t.reactance_ohm));
                        let value = |k: ElementKind| s.stub.filter(|t| t.element.kind == k).map(|t| t.element.value);
                        c.at(w, "uh", value(ElementKind::L));
                        c.at(w, "pf", value(ElementKind::C));
                        c.at(w, "ls", s.stub.map(|t| t.short_wl));
                        c.at(w, "lscm", s.stub.map(|t| t.short_cm));
                        c.at(w, "lo", s.stub.map(|t| t.open_wl));
                        c.at(w, "locm", s.stub.map(|t| t.open_cm));
                        c.at(w, "ri", Some(s.input.re));
                        c.at(w, "xi", Some(s.input.im));
                        c.at(w, "swr", Some(s.swr));
                    }
                }
                Err(e) => c.fail(e),
            }
        }
        "hairpin" => match hairpin(&HairpinInput { resistance_ohm: arg(a, 1), target_ohm: arg(a, 2) }) {
            Ok(h) => c.num("jx", h.reactance_ohm),
            Err(e) => c.fail(e),
        },
        other => panic!("unknown case {other}"),
    }
}

#[test]
fn every_calculator_matches_the_original() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/circuits");
    let cases = std::fs::read_to_string(format!("{root}/cases.txt")).unwrap();
    let golden: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(format!("{root}/golden.json")).unwrap()).unwrap();
    let cases: Vec<&str> =
        cases.lines().map(|l| l.split('#').next().unwrap().trim()).filter(|l| !l.is_empty()).collect();
    assert_eq!(cases.len(), golden.len());
    let mut failures = Vec::new();
    for (case, golden) in cases.iter().zip(&golden) {
        let a: Vec<&str> = case.split_whitespace().collect();
        let mut c = Check { case, golden, failures: Vec::new() };
        check(&mut c, &a);
        failures.extend(c.failures);
    }
    assert!(failures.is_empty(), "{} differences:\n{}", failures.len(), failures.join("\n"));
}
