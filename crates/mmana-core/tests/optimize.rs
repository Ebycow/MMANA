//! Compares the optimiser with the original C++ `CACal` / `CACal::Exec`.
//!
//! `testdata/optimize/golden/NAME.json` is produced by
//! `reference/oracle/gen_optimize.sh` from `testdata/optimize/cases/NAME.txt`
//! (the case format is described in `reference/oracle/optimize_main.cpp`).
//! Each golden file holds the variable table the setup edits made, every
//! calculation of the search (the values tried, the figures and the score),
//! the remembered trials and the final antenna.
//!
//! The values tried are sums and products of the starting values and steps,
//! which IEEE 754 rounds the same everywhere, but the scores come from the
//! engine. On the golden platform everything must be bit-identical; elsewhere
//! a different libm may move a score by an ulp, so figures get a tolerance
//! and the search may part ways where two scores were that close. Edits that
//! go through `sin`/`cos` (the polar form) may also put a coordinate an ulp
//! from the original's (MSVC's libm does); the integration order no longer
//! switches on that (`ORDER_MARGIN`), so the figures stay within the same
//! tolerance.

use mmana_core::optimize::{
    self, Band, ElementAxis, ElementCombination, Link, Matching, OptimizerSetup, SetupOp, StopReason, Variable,
    VariableTarget,
};
use mmana_core::{CalcOptions, GroundKind, Project, Source, maa};
use serde_json::Value;
use std::path::{Path, PathBuf};

const GOLDEN_PLATFORM: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));
const TOL_REL: f64 = 1e-5;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn same(a: f64, b: f64) -> bool {
    same_within(a, b, TOL_REL, 1.0)
}

/// `same` for a figure, with the difference measured against `scale` too
/// (|Z| for R and jX).
fn same_figure(a: f64, b: f64, scale: f64) -> bool {
    same_within(a, b, TOL_REL, scale)
}

fn same_within(a: f64, b: f64, tol: f64, scale: f64) -> bool {
    a.to_bits() == b.to_bits() || (a == 0.0 && b == 0.0) || (!GOLDEN_PLATFORM && close(a, b, tol, scale))
}

fn close(a: f64, b: f64, tol: f64, scale: f64) -> bool {
    a == b || (a - b).abs() <= tol * a.abs().max(b.abs()).max(scale).max(1.0)
}

struct Case {
    project: Project,
    options: CalcOptions,
    setup: OptimizerSetup,
    limit: usize,
}

/// `TypeByName` and the original's `Type` / `Pos` / `SubPos`.
fn target(kind: &str, pos: usize, sub: u8) -> VariableTarget {
    let code = ["wire", "ele", "load", "height", "freq", "wirep", "feed", "stack"]
        .iter()
        .position(|&k| k == kind)
        .unwrap_or_else(|| panic!("unknown variable type {kind}"));
    VariableTarget::from_code(code as u8, pos, sub).unwrap_or_else(|| panic!("bad variable {kind} {pos} {sub}"))
}

fn link(s: &str) -> Link {
    let p: Vec<&str> = s.split(':').collect();
    let variable = || p[1].parse().unwrap();
    let c = || p[2].parse().unwrap();
    match p[0] {
        "none" => Link::Independent,
        "same" => Link::Same { variable: variable() },
        "neg" => Link::Negated { variable: variable() },
        "offset" => Link::Offset { variable: variable(), offset: c() },
        "negoffset" => Link::NegatedOffset { variable: variable(), offset: c() },
        "scaled" => Link::Scaled { variable: variable(), factor: c() },
        "divided" => Link::Divided { variable: variable(), divisor: c() },
        _ => panic!("bad link {s}"),
    }
}

fn axis(n: &str, choices: [ElementAxis; 2]) -> Option<ElementAxis> {
    match n {
        "1" => Some(choices[0]),
        "2" => Some(choices[1]),
        _ => None,
    }
}

/// Reads a case the way optimize_main.cpp does: the settings first, then the
/// setup edits in order.
fn load_case(text: &str) -> Case {
    let lines: Vec<Vec<&str>> =
        text.lines().map(|l| l.split('#').next().unwrap().split_whitespace().collect()).collect();
    let mut setup = OptimizerSetup::default();
    let mut options = CalcOptions::default();
    let mut project = None;
    let mut limit = 400;
    for t in &lines {
        let Some(&key) = t.first() else { continue };
        let a = |i: usize| t[i + 1];
        let n = |i: usize| a(i).parse::<f64>().unwrap();
        let on = |i: usize| a(i) != "0";
        match key {
            "file" => {
                let bytes = std::fs::read(root().join("testdata/maa").join(a(0))).unwrap();
                project = Some(maa::parse_bytes(&bytes).project);
            }
            "freq" => options.frequency_mhz = Some(n(0)),
            "weights" => {
                let w: Vec<u32> = t[1..].iter().map(|s| s.parse().unwrap()).collect();
                let s = &mut setup.weights;
                [s.gain, s.fb, s.jx, s.swr, s.elevation, s.matching, s.current] = w.try_into().unwrap();
            }
            "sweep" => setup.sweep_only = on(0),
            "pitchabs" => setup.absolute_pitch = on(0),
            "coarse" => setup.coarse_pattern = on(0),
            "emphasize" => setup.emphasize_first_band = on(0),
            "maxgain" => setup.goals.max_gain_dbi = n(0),
            "maxfb" => setup.goals.max_fb_db = n(0),
            "minjx" => setup.goals.min_jx = n(0),
            "minswr" => setup.goals.min_swr = n(0),
            "match" => {
                let value = |s: &str| (s != "*").then(|| s.parse().unwrap());
                setup.goals.matching = match a(0) {
                    "0" => Matching::Capacitance,
                    "1" => Matching::Hairpin,
                    _ => Matching::Impedance { r: value(a(1)), x: value(a(2)) },
                };
            }
            "current" => {
                setup.goals.current_pulse = a(0).into();
                setup.goals.minimize_current = t.len() > 2 && on(1);
            }
            "band" => setup.bands.push(Band {
                frequency_mhz: n(0),
                sources: t[2..]
                    .chunks(3)
                    .map(|c| Source {
                        pulse: c[0].into(),
                        phase_deg: c[1].parse().unwrap(),
                        voltage: c[2].parse().unwrap(),
                    })
                    .collect(),
            }),
            "limit" => limit = a(0).parse().unwrap(),
            _ => {}
        }
    }
    let mut project = project.expect("a file line");
    for t in &lines {
        match t.first().copied() {
            Some("ground") => project.environment.ground = GroundKind::from_code(t[1].parse().unwrap()),
            Some("height") => project.environment.height = t[1].parse().unwrap(),
            _ => {}
        }
    }
    for t in &lines {
        let Some(&key) = t.first() else { continue };
        let index = || t[1].parse::<usize>().unwrap();
        let op = match key {
            "set" => SetupOp::SetTarget {
                index: index(),
                target: target(t[2], t[3].parse().unwrap(), t[4].parse().unwrap()),
            },
            "insert" => SetupOp::Insert { index: index() },
            "delete" => SetupOp::Delete { index: index() },
            "clear" => SetupOp::Clear,
            "linksame" => SetupOp::LinkSame { index: index() },
            "linkele" => SetupOp::LinkElements,
            "allele" => {
                let combination = (t.len() > 3).then(|| ElementCombination {
                    width: t[3] != "0",
                    height: t[4] != "0",
                    x_width: t[5] != "0",
                    width_link: axis(t[6], [ElementAxis::Height, ElementAxis::XWidth]),
                    height_link: axis(t[7], [ElementAxis::Width, ElementAxis::XWidth]),
                    x_width_link: axis(t[8], [ElementAxis::Width, ElementAxis::Height]),
                });
                SetupOp::AllElements { intervals: t[1] != "0", scale_all: t[2] != "0", combination }
            }
            "var" => {
                let v = &mut setup.variables[index()];
                for kv in &t[2..] {
                    let (k, val) = kv.split_once('=').unwrap();
                    match k {
                        "pitch" => v.pitch = val.parse().unwrap(),
                        "min" | "max" => {
                            let from = val.strip_prefix('@').map(|s| s.parse().unwrap());
                            let (bound, from_slot) =
                                if k == "min" { (&mut v.min, &mut v.min_from) } else { (&mut v.max, &mut v.max_from) };
                            *from_slot = from;
                            if from.is_none() {
                                *bound = val.parse().unwrap();
                            }
                        }
                        "link" => v.link = link(val),
                        _ => panic!("bad field {kv}"),
                    }
                }
                continue;
            }
            _ => continue,
        };
        let edit = optimize::edit_setup(&project, &options, &setup, &[op]);
        assert_eq!(edit.applied, vec![true], "{key} was refused");
        setup = edit.setup;
    }
    Case { project, options, setup, limit }
}

/// The original's `With`, `DW`, `Min` and `Max` of a variable.
fn original_fields(v: &Variable) -> (i64, f64, i64, i64) {
    let (with, dw) = match v.link {
        Link::Independent => (-1, v.pitch),
        Link::Same { variable } => (variable as i64, v.pitch),
        Link::Negated { variable } => (-(variable as i64) - 2, v.pitch),
        Link::Offset { variable, offset } => (variable as i64 | 0x10000, offset),
        Link::NegatedOffset { variable, offset } => (variable as i64 | 0x20000, offset),
        Link::Scaled { variable, factor } => (variable as i64 | 0x40000, factor),
        Link::Divided { variable, divisor } => (variable as i64 | 0x80000, divisor),
    };
    let flag = |from: Option<usize>| from.map_or(0, |k| k as i64 | 0x8000);
    (with, dw, flag(v.min_from), flag(v.max_from))
}

/// `ToFix`: the 1/256 dB fixed point of the original's trial list.
fn to_fix(d: f32) -> i64 {
    let d = d * 256.0;
    if d <= -32767.0 { -32767 } else { d as i16 as i64 }
}

fn run_case(name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(root().join(format!("testdata/optimize/cases/{name}.txt"))).unwrap();
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join(format!("testdata/optimize/golden/{name}.json"))).unwrap(),
    )
    .unwrap();
    let case = load_case(&text);
    let mut problems = Vec::new();

    // the variable table after the setup edits
    let gvars = golden["variables"].as_array().unwrap();
    if gvars.len() != case.setup.variables.len() {
        problems.push(format!("{} variables, original {}", case.setup.variables.len(), gvars.len()));
        return problems;
    }
    for (i, (v, g)) in case.setup.variables.iter().zip(gvars).enumerate() {
        let (t, p, s) = v.target.code();
        let (with, dw, min_flag, max_flag) = original_fields(v);
        let gi = |k: usize| g[k].as_i64().unwrap();
        let ok = [t as i64, p as i64, s as i64, with] == [gi(0), gi(1), gi(2), gi(3)]
            && same(dw, f(&g[4]))
            && same(v.min, f(&g[5]))
            && same(v.max, f(&g[6]))
            && [min_flag, max_flag] == [gi(7), gi(8)];
        if !ok {
            problems.push(format!("variable {i}: {v:?} != original {g}"));
        }
    }
    if !problems.is_empty() {
        return problems;
    }

    // the search, calculation by calculation
    let mut calcs = Vec::new();
    let out = optimize::optimize(&case.project, &case.options, &case.setup, Some(case.limit), |p| {
        calcs.push(p.clone());
        true
    });
    if let Some(e) = golden.get("error") {
        if out.is_ok() {
            problems.push(format!("the original refuses the setup ({e}); the port runs it"));
        }
        return problems;
    }
    let out = match out {
        Ok(o) => o,
        Err(e) => return vec![format!("refused: {e}")],
    };
    let gcalcs = golden["calcs"].as_array().unwrap();
    let mut diverged = false;
    // a score that differs (within the tolerance) from the original's: the
    // search may part ways from here off the golden platform
    let mut near_tie = false;
    for (k, g) in gcalcs.iter().enumerate() {
        let Some(c) = calcs.get(k) else {
            problems.push(format!("only {} calculations, original {}", calcs.len(), gcalcs.len()));
            break;
        };
        let gvalues: Vec<f64> = g[0].as_array().unwrap().iter().map(f).collect();
        if c.values.len() != gvalues.len() || c.values.iter().zip(&gvalues).any(|(a, b)| a.to_bits() != b.to_bits()) {
            if near_tie && !GOLDEN_PLATFORM {
                diverged = true;
                break;
            }
            problems.push(format!("calculation {}: values {:?} != original {gvalues:?}", k + 1, c.values));
            break;
        }
        let figures = [
            c.frequency_mhz,
            c.impedance.re,
            c.impedance.im,
            c.swr as f64,
            c.gain_dbi as f64,
            c.fb_db as f64,
            c.elevation_deg as f64,
        ];
        let names = ["frequency", "R", "jX", "SWR", "gain", "F/B", "elevation"];
        let z = f(&g[2]).hypot(f(&g[3]));
        for (((a, name), b), scale) in
            figures.iter().zip(names).zip(&g.as_array().unwrap()[1..8]).zip([1.0, z, z, 1.0, 1.0, 1.0, 1.0])
        {
            if !same_figure(*a, f(b), scale) {
                problems.push(format!("calculation {}: {name} {a} != original {}", k + 1, f(b)));
            }
        }
        if let Some(cur) = c.current
            && !same_figure(cur as f64, f(&g[8]), 1.0)
        {
            problems.push(format!("calculation {}: current {cur} != original {}", k + 1, f(&g[8])));
        }
        if let Some(v) = c.score {
            let gv = f(&g[9]);
            if v.to_bits() != gv.to_bits() {
                if !close(v, gv, TOL_REL, 1.0) || GOLDEN_PLATFORM {
                    problems.push(format!("calculation {}: score {v} != original {gv}", k + 1));
                }
                near_tie = true;
            }
        }
        if problems.len() > 8 {
            return problems;
        }
    }
    if diverged {
        return problems;
    }
    if calcs.len() != gcalcs.len() {
        problems.push(format!("{} calculations, original {}", calcs.len(), gcalcs.len()));
    }
    let gresult = golden["result"].as_i64().unwrap();
    let expected = if gresult == 1 { StopReason::Converged } else { StopReason::CalculationLimit };
    if out.stop != expected {
        problems.push(format!("stopped with {:?}, original {expected:?}", out.stop));
    }

    // the best values and the remembered trials
    let best = out.best.as_ref().expect("a best trial");
    if !same_figure(best.score, f(&golden["vm"]), 1.0) {
        problems.push(format!("best score {} != original {}", best.score, f(&golden["vm"])));
    }
    let gad: Vec<f64> = golden["ad"].as_array().unwrap().iter().map(f).collect();
    if best.values.iter().zip(&gad).any(|(a, b)| a.to_bits() != b.to_bits()) {
        problems.push(format!("best values {:?} != original {gad:?}", best.values));
    }
    let gtrials = golden["trials"].as_array().unwrap();
    if out.trials.len() != gtrials.len() {
        problems.push(format!("{} trials kept, original {}", out.trials.len(), gtrials.len()));
    }
    let one_band = case.setup.bands.len() < 2 || case.setup.sweep_only;
    for (k, (t, g)) in out.trials.iter().zip(gtrials).enumerate() {
        let gvalues: Vec<f64> = g["values"].as_array().unwrap().iter().map(f).collect();
        let figures =
            [t.impedance.re, t.impedance.im, t.swr as f64, t.gain_dbi as f64, t.fb_db as f64, t.elevation_deg as f64];
        let gfig = ["r", "jx", "swr", "gain", "fb", "elv"].map(|n| f(&g[n]));
        let z = gfig[0].hypot(gfig[1]);
        if t.values.iter().zip(&gvalues).any(|(a, b)| a.to_bits() != b.to_bits())
            || !same_figure(t.score, f(&g["v"]), 1.0)
            // (printed as `float` with 9 digits)
            || figures
                .iter()
                .zip(&gfig)
                .zip([z, z, 1.0, 1.0, 1.0, 1.0])
                .any(|((a, b), scale)| !same_figure(*a as f32 as f64, *b as f32 as f64, scale))
        {
            problems.push(format!("trial {k}: {:?} {} {figures:?} != original {g}", t.values, t.score));
            break;
        }
        // the original keeps the last band's patterns; the port band 1's
        if one_band && GOLDEN_PLATFORM {
            let h: Vec<i64> = t.horizontal_pattern.iter().map(|&d| to_fix(d)).collect();
            let v: Vec<i64> = t.vertical_pattern.iter().map(|&d| to_fix(d)).collect();
            let gh: Vec<i64> = g["h"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
            let gv: Vec<i64> = g["v2"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
            if h != gh || v != gv {
                problems.push(format!("trial {k}: patterns differ"));
                break;
            }
        }
    }

    // the antenna with the best values
    let ga = &golden["antenna"];
    let ant = &out.project.antenna;
    for (i, (w, g)) in ant.wires.iter().zip(ga["wires"].as_array().unwrap()).enumerate() {
        let mine = [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z, w.radius];
        if mine.iter().zip(g.as_array().unwrap()).any(|(a, b)| !same(*a, f(b))) {
            problems.push(format!("wire {}: {mine:?} != original {g}", i + 1));
        }
    }
    for (i, (l, g)) in ant.loads.iter().zip(ga["loads"].as_array().unwrap()).enumerate() {
        let (a, b) = match &l.kind {
            mmana_core::LoadKind::Lc { inductance_uh, capacitance_pf, .. } => (*inductance_uh, *capacitance_pf),
            mmana_core::LoadKind::Impedance { r, x } => (*r, *x),
            mmana_core::LoadKind::SDomain { coefficients } => (coefficients[0][0], coefficients[0][1]),
        };
        if !same(a, f(&g[0])) || !same(b, f(&g[1])) {
            problems.push(format!("load {}: {a}, {b} != original {g}", i + 1));
        }
    }
    if one_band {
        for (i, (s, g)) in ant.sources.iter().zip(ga["sources"].as_array().unwrap()).enumerate() {
            if !same(s.phase_deg, f(&g[0])) || !same(s.voltage, f(&g[1])) {
                problems.push(format!("source {}: {} {} != original {g}", i + 1, s.phase_deg, s.voltage));
            }
        }
        if !same(out.frequency_mhz, f(&ga["freq"])) {
            problems.push(format!("frequency {} != original {}", out.frequency_mhz, f(&ga["freq"])));
        }
    }
    let env = &out.project.environment;
    let stack = &ant.stack;
    if !same(env.height, f(&ga["height"]))
        || !same(stack.horizontal_spacing, f(&ga["stack"][0]))
        || !same(stack.vertical_spacing, f(&ga["stack"][1]))
    {
        problems.push(format!(
            "height / stack {} {} {} != original {}",
            env.height, stack.horizontal_spacing, stack.vertical_spacing, ga
        ));
    }
    problems
}

#[test]
fn optimizer_matches_original() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("testdata/optimize/cases"))
        .unwrap()
        .map(|e| e.unwrap().path().file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(!names.is_empty());
    let failures: Vec<String> =
        names.iter().flat_map(|n| run_case(n).into_iter().map(move |p| format!("{n}: {p}"))).collect();
    assert!(failures.is_empty(), "{} problem(s):\n{}", failures.len(), failures.join("\n"));
}
