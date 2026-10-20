//! Compares the element editor with the original C++ `CWGroup` / `CWGroupList`.
//!
//! `testdata/elements/golden/STEM.json` is produced by
//! `reference/oracle/gen_elements.sh`: every sample in `testdata/maa` run
//! through every edit script in `testdata/elements/scripts` (the script format
//! is described in `reference/oracle/elements_main.cpp`). The editor only adds,
//! multiplies, divides and takes square roots, which IEEE 754 rounds the same
//! everywhere, so every value must be bit-identical on every platform.

use mmana_core::elements::{ElementKind, ElementList, ElementOp, ElementRow};
use mmana_core::{SPEED_OF_LIGHT_MMANA, Segmentation, maa};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// Equal to the last bit (`-0` is printed as `-0` and read back as `-0.0`).
fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a == 0.0 && b == 0.0)
}

fn kind_by_name(s: &str) -> ElementKind {
    use ElementKind::*;
    match s {
        "hdp" => HorizontalDipole,
        "vdp" => VerticalDipole,
        "vsq" => VerticalSquare,
        "vdm" => VerticalDiamond,
        "vita" => VerticalInvertedTriangle,
        "vta" => VerticalTriangle,
        "vpie" => VerticalOctagon,
        "vsq2" => VerticalDoubleSquare,
        "vta2" => VerticalDoubleTriangle,
        "hsq" => HorizontalSquare,
        "hdm" => HorizontalDiamond,
        // the cwHTA template is recognised as 水平▽ and cwHITA as 水平△;
        // `ElementKind` names a template by what it builds
        "hta" => HorizontalInvertedTriangle,
        "hita" => HorizontalTriangle,
        _ => panic!("unknown kind {s}"),
    }
}

/// The value an edit starts from (`Current` in elements_main.cpp).
fn current(op: &str, row: Option<&ElementRow>) -> f64 {
    let Some(r) = row else { return 0.0 };
    match op {
        "width" | "widthall" => r.width,
        "height" | "heightall" => r.height,
        "round" | "roundall" => r.perimeter,
        // the original reads the first wire even when the radii differ; the
        // edit is refused then, so the value does not matter
        "radius" => r.radius.unwrap_or(0.0),
        "interval" => r.interval,
        "position" => r.position,
        _ => 0.0,
    }
}

fn op_for(op: &str, row: usize, arg: &str, list: &ElementList) -> ElementOp {
    match op {
        "kind" => return ElementOp::SetKind { row, kind: kind_by_name(arg) },
        "insert" => return ElementOp::Insert { row },
        "delete" => return ElementOp::Delete { row },
        "seg" => {
            return ElementOp::SetSegmentation { row, segmentation: Segmentation::from_code(arg.parse().unwrap()) };
        }
        _ => {}
    }
    let rows = list.rows();
    let cur = current(op, rows.get(row));
    let value = if let Some(k) = arg.strip_prefix('*') {
        cur * k.parse::<f64>().unwrap()
    } else if let Some(d) = arg.strip_prefix('+') {
        cur + d.parse::<f64>().unwrap()
    } else {
        arg.parse().unwrap()
    };
    match op {
        "width" => ElementOp::SetWidth { row, value, scale_all: false },
        "widthall" => ElementOp::SetWidth { row, value, scale_all: true },
        "height" => ElementOp::SetHeight { row, value, scale_all: false },
        "heightall" => ElementOp::SetHeight { row, value, scale_all: true },
        "round" => ElementOp::SetPerimeter { row, value, scale_all: false },
        "roundall" => ElementOp::SetPerimeter { row, value, scale_all: true },
        "radius" => ElementOp::SetRadius { row, value },
        "interval" => ElementOp::SetInterval { row, value },
        "position" => ElementOp::SetPosition { row, value },
        _ => panic!("unknown operation {op}"),
    }
}

/// Runs a script the way elements_main.cpp does; returns whether each edit applied.
fn run_script(list: &mut ElementList, script: &str) -> Vec<bool> {
    let mut applied = Vec::new();
    for line in script.lines() {
        let line = line.split('#').next().unwrap();
        let t: Vec<&str> = line.split_whitespace().collect();
        let Some(&op) = t.first() else { continue };
        if op == "roundup" {
            let flag = |s: &str| t[2..].contains(&s);
            let op = ElementOp::RoundValues {
                decimals: t[1].parse().unwrap(),
                in_wavelengths: flag("l"),
                scale_all: flag("all"),
            };
            applied.push(list.apply(&op));
            continue;
        }
        let len = list.len() as i64;
        let (first, last) = match t[1] {
            "*" => (0, len - 1),
            "new" => (len, len),
            "last" => (len - 1, len - 1),
            "base" => (list.base() as i64, list.base() as i64),
            n => (n.parse().unwrap(), n.parse().unwrap()),
        };
        let arg = t.get(2).copied().unwrap_or("");
        for r in first..=last {
            let ok = r >= 0 && r <= list.len() as i64 && {
                let op = op_for(op, r as usize, arg, list);
                list.apply(&op)
            };
            applied.push(ok);
        }
    }
    applied
}

fn compare(list: &ElementList, applied: &[bool], g: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let gapplied: Vec<bool> = g["applied"].as_array().unwrap().iter().map(|v| v.as_i64() == Some(1)).collect();
    if applied != gapplied {
        problems.push(format!("applied {applied:?} != {gapplied:?}"));
    }
    let dup = list.duplicate().map(|(a, b)| vec![a as u64, b as u64]);
    let gdup = g["same"].as_array().map(|v| v.iter().map(|x| x.as_u64().unwrap()).collect::<Vec<_>>());
    if dup != gdup {
        problems.push(format!("duplicate {dup:?} != {gdup:?}"));
    }
    if list.base() as u64 != g["base"].as_u64().unwrap() {
        problems.push(format!("base {} != {}", list.base(), g["base"]));
    }
    let rows = list.rows();
    let grows = g["rows"].as_array().unwrap();
    if rows.len() != grows.len() {
        problems.push(format!("{} elements != {}", rows.len(), grows.len()));
        return problems;
    }
    for (i, (r, gr)) in rows.iter().zip(grows).enumerate() {
        let nums = [
            ("interval", r.interval),
            ("position", r.position),
            ("width", r.width),
            ("height", r.height),
            ("round", r.perimeter),
        ];
        for (k, v) in nums {
            if !same(v, f(&gr[k])) {
                problems.push(format!("row {i} {k} {v:?} != {}", gr[k]));
            }
        }
        if r.kind as i64 != gr["type"].as_i64().unwrap() {
            problems.push(format!("row {i} kind {:?} != {}", r.kind, gr["type"]));
        }
        let flag = |k: &str| gr[k].as_i64() == Some(1);
        if r.x_width != flag("xw") || r.radius.is_some() != flag("enbR") || r.segmentation.is_some() != flag("enbSeg") {
            problems.push(format!("row {i} flags differ: {r:?} vs {gr}"));
        }
        if r.radius.is_some_and(|v| !same(v, f(&gr["r"]))) {
            problems.push(format!("row {i} radius {:?} != {}", r.radius, gr["r"]));
        }
        if r.segmentation.is_some_and(|s| s.to_code() as i64 != gr["seg"].as_i64().unwrap()) {
            problems.push(format!("row {i} seg {:?} != {}", r.segmentation, gr["seg"]));
        }
        if r.wire_count as u64 != gr["wires"].as_u64().unwrap() {
            problems.push(format!("row {i} wires {} != {}", r.wire_count, gr["wires"]));
        }
    }
    let wires = list.wires().wires;
    let gwires = g["wires"].as_array().unwrap();
    if wires.len() != gwires.len() {
        problems.push(format!("{} wires != {}", wires.len(), gwires.len()));
        return problems;
    }
    for (i, (w, gw)) in wires.iter().zip(gwires).enumerate() {
        let ours = [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z, w.radius];
        let gw = gw.as_array().unwrap();
        if !ours.iter().zip(gw).all(|(a, b)| same(*a, f(b)))
            || w.segmentation.to_code() as i64 != gw[7].as_i64().unwrap()
        {
            problems.push(format!("wire {i} {ours:?} seg {} != {gw:?}", w.segmentation.to_code()));
        }
    }
    problems
}

#[test]
fn element_editor_matches_original() {
    let scripts_dir = root().join("testdata/elements/scripts");
    let mut scripts: Vec<(String, String)> = std::fs::read_dir(&scripts_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .map(|p| (p.file_stem().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&p).unwrap()))
        .collect();
    scripts.sort();
    let mut goldens: Vec<PathBuf> = std::fs::read_dir(root().join("testdata/elements/golden"))
        .expect("testdata/elements/golden (run reference/oracle/gen_elements.sh)")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    goldens.sort();
    assert!(goldens.len() >= 81, "expected a golden file per sample, found {}", goldens.len());

    let mut failures = Vec::new();
    let mut cases = 0;
    for path in &goldens {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let golden: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let bytes = std::fs::read(root().join(format!("testdata/maa/{stem}.MAA"))).unwrap();
        let antenna = maa::parse_bytes(&bytes).project.antenna;
        let fq = antenna.frequency_mhz;
        let wavelength = if fq != 0.0 { SPEED_OF_LIGHT_MMANA / fq } else { 0.0 };
        for (name, script) in &scripts {
            let g = &golden[name];
            assert!(!g.is_null(), "{stem}: no result for script {name}");
            cases += 1;
            let mut problems = Vec::new();
            if !same(wavelength, f(&g["rmd"])) {
                problems.push(format!("wavelength {wavelength} != {}", g["rmd"]));
            }
            let mut list = ElementList::new(&antenna.wires, wavelength);
            let applied = run_script(&mut list, script);
            problems.extend(compare(&list, &applied, g));
            if !problems.is_empty() {
                problems.truncate(5);
                failures.push(format!("{stem} [{name}]:\n    {}", problems.join("\n    ")));
            }
        }
    }
    assert!(failures.is_empty(), "{} of {cases} cases differ:\n{}", failures.len(), failures.join("\n"));
}
