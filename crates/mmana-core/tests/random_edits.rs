//! Random sequences of element edits on the sample antennas and on small
//! random wire sets. Whatever the edits, the editor must not panic, an edit
//! it refuses must change nothing, and an edit typed back to its old value
//! must give the antenna back; edits that keep a sample's shape sensible
//! must give an antenna that still calculates.
//!
//! The default 256 cases run with `cargo test`; `PROPTEST_CASES=20000` runs
//! the long version (`.github/workflows/quality.yml` does so every night).
//! Failures are saved in `random_edits.proptest-regressions`.

// `!(off <= tol)` fails on NaN too
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use mmana_core::elements::{ElementEdit, ElementKind, ElementList, ElementOp, ElementRow, edit_antenna};
use mmana_core::{
    Antenna, CalcError, CalcOptions, Point3, Project, SPEED_OF_LIGHT_MMANA, Segmentation, Wire, calculate, maa,
    pulse_layout,
};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use serde_json::Value;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::OnceLock;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

/// Refuses any single allocation over 256 MB (see random_models.rs): an edit
/// that blows a size up stops the test binary instead of filling memory.
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

const LIMIT: Duration = Duration::from_secs(20);

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

/// The sample antennas.
fn samples() -> &'static [Project] {
    static SAMPLES: OnceLock<Vec<Project>> = OnceLock::new();
    SAMPLES.get_or_init(|| {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa");
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        paths.iter().map(|p| maa::parse_bytes(&std::fs::read(p).unwrap()).project).collect()
    })
}

fn wavelength(antenna: &Antenna) -> f64 {
    let f = antenna.frequency_mhz;
    if f != 0.0 { SPEED_OF_LIGHT_MMANA / f } else { 0.0 }
}

/// A few wires on a lattice, so that they join into loops and bent elements.
fn wires() -> impl Strategy<Value = Vec<Wire>> {
    let point = (prop::sample::select(vec![-2.0, 0.0, 2.0]), -2..3i32, -2..3i32)
        .prop_map(|(x, y, z)| Point3::new(x, y as f64 * 2.5, z as f64 * 2.5));
    let radius = prop_oneof![1 => Just(0.0), 1 => Just(-1.0), 8 => Just(0.001)];
    let segmentation =
        prop_oneof![Just(Segmentation::Auto), (1..12i32).prop_map(|count| Segmentation::Fixed { count })];
    let wire = (point.clone(), point, radius, segmentation).prop_map(|(start, end, radius, segmentation)| Wire {
        start,
        end,
        radius,
        segmentation,
        note: String::new(),
        group: String::new(),
    });
    prop::collection::vec(wire, 0..7)
}

/// A sample, or random wires at 14.05 MHz with a source on the first.
fn antenna() -> impl Strategy<Value = Antenna> {
    prop_oneof![
        3 => (0..samples().len()).prop_map(|i| samples()[i].antenna.clone()),
        1 => wires().prop_map(|wires| {
            let mut a = samples().iter().find(|p| p.antenna.name.contains("DP")).unwrap_or(&samples()[0]).antenna.clone();
            a.wires = wires;
            a
        }),
    ]
}

/// A value in an edit: relative to what the cell shows, or a raw value.
#[derive(Debug, Clone, Copy)]
enum Val {
    Times(f64),
    Plus(f64),
    Raw(f64),
}

impl Val {
    fn of(self, current: f64) -> f64 {
        match self {
            Val::Times(k) => current * k,
            Val::Plus(d) => current + d,
            Val::Raw(v) => v,
        }
    }
}

fn val() -> impl Strategy<Value = Val> {
    let raw = prop::sample::select(vec![
        0.0,
        -0.0,
        -1.0,
        1e-9,
        1e-300,
        1e6,
        1e300,
        f64::MAX,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ]);
    prop_oneof![
        4 => (0.2..5.0f64).prop_map(Val::Times),
        2 => (-3.0..3.0f64).prop_map(Val::Plus),
        2 => raw.prop_map(Val::Raw),
    ]
}

#[derive(Debug, Clone, Copy)]
enum Cell {
    Interval,
    Position,
    Width,
    Height,
    Perimeter,
    Radius,
}

impl Cell {
    fn of(self, r: &ElementRow) -> f64 {
        match self {
            Cell::Interval => r.interval,
            Cell::Position => r.position,
            Cell::Width => r.width,
            Cell::Height => r.height,
            Cell::Perimeter => r.perimeter,
            Cell::Radius => r.radius.unwrap_or(0.0),
        }
    }

    fn op(self, row: usize, value: f64, scale_all: bool) -> ElementOp {
        match self {
            Cell::Interval => ElementOp::SetInterval { row, value },
            Cell::Position => ElementOp::SetPosition { row, value },
            Cell::Width => ElementOp::SetWidth { row, value, scale_all },
            Cell::Height => ElementOp::SetHeight { row, value, scale_all },
            Cell::Perimeter => ElementOp::SetPerimeter { row, value, scale_all },
            Cell::Radius => ElementOp::SetRadius { row, value },
        }
    }
}

/// An edit before it meets an antenna: the row is picked among the rows
/// there are then, plus the "新規" row and one past it.
#[derive(Debug, Clone)]
enum Step {
    Set { row: usize, cell: Cell, value: Val, scale_all: bool },
    Segmentation { row: usize, segmentation: Segmentation },
    Kind { row: usize, kind: ElementKind },
    Insert { row: usize },
    Delete { row: usize },
    Round { decimals: u32, in_wavelengths: bool, in_millimetres: bool, scale_all: bool },
}

const KINDS: [ElementKind; 14] = [
    ElementKind::HorizontalDipole,
    ElementKind::VerticalDipole,
    ElementKind::VerticalSquare,
    ElementKind::VerticalDiamond,
    ElementKind::VerticalTriangle,
    ElementKind::VerticalInvertedTriangle,
    ElementKind::VerticalOctagon,
    ElementKind::VerticalDoubleSquare,
    ElementKind::VerticalDoubleTriangle,
    ElementKind::HorizontalSquare,
    ElementKind::HorizontalDiamond,
    ElementKind::HorizontalTriangle,
    ElementKind::HorizontalInvertedTriangle,
    ElementKind::VerticalLoop,
];

fn segmentation() -> impl Strategy<Value = Segmentation> {
    prop_oneof![
        (1..30i32).prop_map(|count| Segmentation::Fixed { count }),
        Just(Segmentation::Auto),
        Just(Segmentation::Taper),
        Just(Segmentation::TaperStart),
        Just(Segmentation::TaperEnd),
        (1..400i32, prop::option::of(1..80i32)).prop_map(|(dm1, dm2)| Segmentation::TaperCustom { dm1, dm2 }),
        Just(Segmentation::Fixed { count: 0 }),
        Just(Segmentation::Fixed { count: -7 }),
    ]
}

fn cell() -> impl Strategy<Value = Cell> {
    prop::sample::select(vec![Cell::Interval, Cell::Position, Cell::Width, Cell::Height, Cell::Perimeter, Cell::Radius])
}

fn step() -> impl Strategy<Value = Step> {
    let row = 0..64usize;
    prop_oneof![
        8 => (row.clone(), cell(), val(), any::<bool>())
            .prop_map(|(row, cell, value, scale_all)| Step::Set { row, cell, value, scale_all }),
        1 => (row.clone(), segmentation()).prop_map(|(row, segmentation)| Step::Segmentation { row, segmentation }),
        1 => (row.clone(), prop::sample::select(KINDS.to_vec())).prop_map(|(row, kind)| Step::Kind { row, kind }),
        1 => row.clone().prop_map(|row| Step::Insert { row }),
        1 => row.prop_map(|row| Step::Delete { row }),
        1 => (0..12u32, any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(decimals, in_wavelengths, in_millimetres, scale_all)| {
            Step::Round { decimals, in_wavelengths, in_millimetres, scale_all }
        }),
    ]
}

/// An edit that keeps an antenna sensible: sizes and intervals within 0.8 to
/// 1.25 times, radii within 0.5 to 2 times, a few segmentations.
fn sane_step() -> impl Strategy<Value = Step> {
    let row = 0..64usize;
    let set = |cells: Vec<Cell>, lo: f64, hi: f64| {
        (0..64usize, prop::sample::select(cells), lo..hi, any::<bool>())
            .prop_map(|(row, cell, k, scale_all)| Step::Set { row, cell, value: Val::Times(k), scale_all })
    };
    let segmentation = prop_oneof![
        (5..22i32).prop_map(|count| Segmentation::Fixed { count }),
        Just(Segmentation::Auto),
        Just(Segmentation::Taper),
    ];
    prop_oneof![
        4 => set(vec![Cell::Width, Cell::Height, Cell::Perimeter, Cell::Interval], 0.8, 1.25),
        1 => set(vec![Cell::Radius], 0.5, 2.0),
        1 => (row, segmentation).prop_map(|(row, segmentation)| Step::Segmentation { row, segmentation }),
    ]
}

/// The samples that calculate as they are, at most 1500 pulses.
fn calculable() -> &'static [usize] {
    static OK: OnceLock<Vec<usize>> = OnceLock::new();
    OK.get_or_init(|| {
        (0..samples().len())
            .filter(|&i| calculate(&samples()[i], &options()).is_ok_and(|r| r.warnings.is_empty()))
            .collect()
    })
}

fn options() -> CalcOptions {
    CalcOptions { max_pulses: 1500, ..CalcOptions::default() }
}

/// The edit `step` makes on the editor as it is now (as the dialog would
/// send it: values typed relative to what the cells show).
fn concrete(step: &Step, list: &ElementList) -> ElementOp {
    let rows = list.rows();
    let pick = |row: usize| row % (rows.len() + 2);
    match *step {
        Step::Set { row, cell, value, scale_all } => {
            let row = pick(row);
            let current = rows.get(row).map_or(0.0, |r| cell.of(r));
            cell.op(row, value.of(current), scale_all)
        }
        Step::Segmentation { row, segmentation } => ElementOp::SetSegmentation { row: pick(row), segmentation },
        Step::Kind { row, kind } => ElementOp::SetKind { row: pick(row), kind },
        Step::Insert { row } => ElementOp::Insert { row: pick(row) },
        Step::Delete { row } => ElementOp::Delete { row: pick(row) },
        Step::Round { decimals, in_wavelengths, in_millimetres, scale_all } => {
            ElementOp::RoundValues { decimals, in_wavelengths, in_millimetres, scale_all }
        }
    }
}

/// The edits `steps` make on `antenna`, in order.
fn concrete_ops(antenna: &Antenna, steps: &[Step]) -> Vec<ElementOp> {
    let mut list = ElementList::new(&antenna.wires, wavelength(antenna));
    steps
        .iter()
        .map(|s| {
            let op = concrete(s, &list);
            list.apply(&op);
            op
        })
        .collect()
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

/// The non-finite numbers in the element table and the wires.
fn non_finite(e: &ElementEdit) -> Vec<String> {
    let mut found = Vec::new();
    for (i, r) in e.elements.iter().enumerate() {
        let cells = [r.interval, r.position, r.width, r.height, r.perimeter, r.radius.unwrap_or(0.0)];
        if !cells.iter().all(|c| c.is_finite()) {
            found.push(format!("element {i}: {cells:?}"));
        }
    }
    for (i, w) in e.antenna.wires.iter().enumerate() {
        let p = [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z, w.radius];
        if !p.iter().all(|c| c.is_finite()) {
            found.push(format!("wire {}: {p:?}", i + 1));
        }
    }
    found
}

/// The largest coordinate of the wires, for a tolerance in proportion.
fn extent(wires: &[Wire]) -> f64 {
    wires
        .iter()
        .flat_map(|w| [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z])
        .fold(1.0, |m, c| m.max(c.abs()))
}

/// The wires are the same up to rounding of `tol`.
fn same_wires(a: &[Wire], b: &[Wire], tol: f64) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!("{} wires, expected {}", a.len(), b.len()));
    }
    for (i, (w, v)) in a.iter().zip(b).enumerate() {
        let p = [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z];
        let q = [v.start.x, v.start.y, v.start.z, v.end.x, v.end.y, v.end.z];
        let off = p.iter().zip(&q).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        if !(off <= tol) || w.radius != v.radius || w.segmentation != v.segmentation {
            return Err(format!("wire {}: {w:?}, expected {v:?} (off by {off:e})", i + 1));
        }
    }
    Ok(())
}

/// Where the table differs from the wires: the extent along Y and Z of each
/// element's wires against its width and height, and the distance between
/// the centres along X against the positions. Sizes may differ by 10 ppm of
/// the element, at least 0.01 mm (what `ElementList::apply` allows); `round` is the rounding
/// of coordinates the size of the antenna.
fn table_mismatch(e: &ElementEdit, round: f64) -> Vec<String> {
    let mut lo = vec![[f64::INFINITY; 3]; e.elements.len()];
    let mut hi = vec![[f64::NEG_INFINITY; 3]; e.elements.len()];
    for (w, &row) in e.antenna.wires.iter().zip(&e.wire_elements) {
        for p in [w.start, w.end] {
            for (axis, c) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[row][axis] = lo[row][axis].min(c);
                hi[row][axis] = hi[row][axis].max(c);
            }
        }
    }
    let tol: Vec<f64> = (0..e.elements.len())
        .map(|i| round + (1e-5 * (0..3).fold(0.0, |m: f64, a| m.max(hi[i][a] - lo[i][a]))).max(1e-5))
        .collect();
    let centre = |row: usize| lo[row][0] / 2.0 + hi[row][0] / 2.0;
    // (the position of an element with depth along X is that of its loop,
    // which the extent of all its wires does not tell)
    let flat: Vec<usize> = (0..e.elements.len()).filter(|&i| hi[i][0] - lo[i][0] <= round).collect();
    let mut off = Vec::new();
    for (i, r) in e.elements.iter().enumerate() {
        let (y, z) = (hi[i][1] - lo[i][1], hi[i][2] - lo[i][2]);
        if y > tol[i] && !((r.width - y).abs() <= tol[i]) {
            off.push(format!("element {i}: width {} but the wires span {y}", r.width));
        }
        if z > tol[i] && !((r.height - z).abs() <= tol[i]) {
            off.push(format!("element {i}: height {} but the wires span {z}", r.height));
        }
        if let Some(&j) = flat.first()
            && flat.contains(&i)
        {
            let apart = centre(i) - centre(j);
            if !((r.position - e.elements[j].position - apart).abs() <= tol[i] + tol[j]) {
                off.push(format!(
                    "element {i}: {} from element {j} but the wires {apart}",
                    r.position - e.elements[j].position
                ));
            }
        }
    }
    off
}

/// Where the wires of element `row` lie along Y and Z: from 0, to 0,
/// across 0 or off it (which decides how the original resizes it).
fn crossing(e: &ElementEdit, row: usize) -> [u8; 2] {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for (w, _) in e.antenna.wires.iter().zip(&e.wire_elements).filter(|(_, r)| **r == row) {
        for p in [w.start, w.end] {
            for (k, c) in [p.y, p.z].into_iter().enumerate() {
                lo[k] = lo[k].min(c);
                hi[k] = hi[k].max(c);
            }
        }
    }
    [0, 1].map(|k| match (lo[k], hi[k]) {
        (0.0, _) => 0,
        (_, 0.0) => 1,
        (l, h) if l < 0.0 && h > 0.0 => 2,
        _ => 3,
    })
}

/// The element tables are the same up to rounding of `tol`.
fn same_rows(a: &[ElementRow], b: &[ElementRow], tol: f64) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!("{} elements, expected {}", a.len(), b.len()));
    }
    for (i, (r, s)) in a.iter().zip(b).enumerate() {
        let cells = [Cell::Interval, Cell::Position, Cell::Width, Cell::Height, Cell::Perimeter];
        let off = cells.iter().map(|c| (c.of(r) - c.of(s)).abs()).fold(0.0, f64::max);
        if !(off <= tol)
            || r.kind != s.kind
            || r.radius != s.radius
            || r.segmentation != s.segmentation
            || r.wire_count != s.wire_count
        {
            return Err(format!("element {i}: {r:?}, expected {s:?}"));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn any_element_edits_end_without_a_panic_and_refused_ones_change_nothing(
        antenna in antenna(),
        steps in prop::collection::vec(step(), 0..8),
    ) {
        let (a, s) = (antenna.clone(), steps.clone());
        let (ops, edit) = within(&antenna, move || {
            let ops = concrete_ops(&a, &s);
            let edit = edit_antenna(&a, &ops);
            (ops, edit)
        });
        prop_assert_eq!(edit.applied.len(), ops.len());
        // an edit that would make a size or a coordinate overflow is refused
        let bad = non_finite(&edit);
        prop_assert!(bad.is_empty(), "non-finite values at {:?}", &bad[..bad.len().min(5)]);
        // nothing applied (or undone to the start): the antenna as it was
        if !edit.applied.contains(&true) {
            prop_assert_eq!(&edit.antenna, &antenna);
        }
        // the dialog drops refused edits from its undo steps
        let kept: Vec<ElementOp> = ops.iter().zip(&edit.applied).filter(|(_, ok)| **ok).map(|(op, _)| op.clone()).collect();
        let replayed = edit_antenna(&antenna, &kept);
        prop_assert!(replayed.applied.iter().all(|&ok| ok), "a kept edit was refused on replay: {:?}", replayed.applied);
        prop_assert_eq!(&replayed.antenna, &edit.antenna);
        prop_assert_eq!(&replayed.elements, &edit.elements);
        prop_assert_eq!(replayed.base, edit.base);
        prop_assert_eq!(replayed.duplicate, edit.duplicate);
        prop_assert_eq!(&replayed.wire_elements, &edit.wire_elements);
        // one element per wire, a base among the elements
        prop_assert_eq!(edit.wire_elements.len(), edit.antenna.wires.len());
        prop_assert!(edit.wire_elements.iter().all(|&e| e < edit.elements.len()));
        prop_assert!(edit.elements.is_empty() || edit.base < edit.elements.len());
        let wires_per_element: usize = edit.elements.iter().map(|r| r.wire_count).sum();
        prop_assert_eq!(wires_per_element, edit.antenna.wires.len());
        // the sizes and positions the table shows are those of the wires it
        // wrote, if they were when the editor opened
        let described = table_mismatch(&edit_antenna(&antenna, &[]), 1e-9 * extent(&antenna.wires)).is_empty();
        if edit.applied.contains(&true) && described {
            let off = table_mismatch(&edit, 1e-9 * extent(&edit.antenna.wires));
            prop_assert!(off.is_empty(), "{off:?}\nops {ops:?}\nrows {:#?}\nwires {:#?}", edit.elements, edit.antenna.wires);
        }
        // sources and loads named by wire name a wire that exists
        for pulse in edit.antenna.sources.iter().map(|s| &s.pulse).chain(edit.antenna.loads.iter().map(|l| &l.pulse)) {
            if let Some(n) = pulse.strip_prefix('w').and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse::<usize>().ok()) {
                let before = antenna.wires.len();
                prop_assert!(n <= edit.antenna.wires.len() || n > before, "{pulse} with {} wires", edit.antenna.wires.len());
            }
        }
    }

    #[test]
    fn a_value_typed_back_gives_the_element_back(
        antenna in antenna(),
        steps in prop::collection::vec(step(), 0..4),
        row in 0..64usize,
        cell in cell(),
        factor in 0.5..2.0f64,
        scale_all in any::<bool>(),
    ) {
        let ops = concrete_ops(&antenna, &steps);
        let before = edit_antenna(&antenna, &ops);
        if before.elements.is_empty() {
            return Ok(());
        }
        let row = row % before.elements.len();
        let old = cell.of(&before.elements[row]);
        let mut tried = ops.clone();
        tried.push(cell.op(row, old * factor, scale_all));
        tried.push(cell.op(row, old, scale_all));
        let after = edit_antenna(&antenna, &tried);
        let [.., changed, back] = after.applied[..] else { unreachable!() };
        // typing the old value back gives the element back, or is refused
        // (where the original would write another shape); the dialog's
        // undo replays the edits. Once an end crosses 0, the original
        // resizes in proportion about 0 instead of about the centre, so the
        // way back lands elsewhere; that is kept (docs/porting-notes.md)
        let mid = edit_antenna(&antenna, &tried[..tried.len() - 1]);
        if !(changed && back) || crossing(&before, row) != crossing(&mid, row) {
            return Ok(());
        }
        let tol = (1e-5 * extent(&before.antenna.wires)).max(1e-5);
        // the original makes an octagon of a new perimeter regular, so one
        // that a width or height made irregular stays regular
        let e = &before.elements[row];
        if matches!(cell, Cell::Perimeter) && e.kind == ElementKind::VerticalOctagon && (e.width - e.height).abs() > tol {
            return Ok(());
        }
        let same = same_rows(&after.elements, &before.elements, tol)
            .and_then(|()| same_wires(&after.antenna.wires, &before.antenna.wires, tol));
        if let Err(e) = same {
            prop_assert!(false, "{cell:?} of element {row} {old} -> {} -> {old}: {e}\nafter {ops:?}", old * factor);
        }
    }

    #[test]
    fn sensible_edits_of_a_sample_keep_it_calculable(
        sample in prop::sample::select(calculable().to_vec()),
        steps in prop::collection::vec(sane_step(), 1..6),
    ) {
        let project = samples()[sample].clone();
        // edits of the elements there are (an edit in the new row makes a
        // copy in the same place); a negative radius names a combined wire
        let mut list = ElementList::new(&project.antenna.wires, wavelength(&project.antenna));
        let mut ops = Vec::new();
        for step in &steps {
            let op = concrete(step, &list);
            let row = match op {
                ElementOp::SetInterval { row, .. }
                | ElementOp::SetWidth { row, .. }
                | ElementOp::SetHeight { row, .. }
                | ElementOp::SetPerimeter { row, .. }
                | ElementOp::SetRadius { row, .. }
                | ElementOp::SetSegmentation { row, .. } => row,
                _ => unreachable!("not a sensible step"),
            };
            if row < list.len() && !matches!(op, ElementOp::SetRadius { value, .. } if value < 0.0) {
                list.apply(&op);
                ops.push(op);
            }
        }
        let edit = edit_antenna(&project.antenna, &ops);
        if !edit.applied.contains(&true) || edit.duplicate.is_some() {
            return Ok(());
        }
        // no element was added or removed: sources and loads keep their names
        prop_assert_eq!(&edit.antenna.sources, &project.antenna.sources);
        prop_assert_eq!(&edit.antenna.loads, &project.antenna.loads);
        let edited = Project { antenna: edit.antenna.clone(), ..project.clone() };
        let e = edited.clone();
        let result = within(&edited, move || calculate(&e, &options()));
        // fewer segments may leave no pulse where a name like `w1c-8` points
        let resegmented = ops.iter().any(|op| matches!(op, ElementOp::SetSegmentation { .. }));
        if resegmented && matches!(result, Err(CalcError::PulseOutOfRange { .. })) {
            return Ok(());
        }
        let r = result.map_err(|e| TestCaseError::fail(format!("{}: rejected after {ops:?}: {e}", project.antenna.name)))?;
        let mut bad = Vec::new();
        nulls(&serde_json::to_value(&r).unwrap(), "", &mut bad);
        prop_assert!(bad.is_empty(), "non-finite at {:?}", &bad[..bad.len().min(5)]);
        // (too coarse a segmentation next to short wires breaks MININEC down;
        // the result says so)
        if r.total_power_w <= 0.0 {
            prop_assert!(!r.warnings.is_empty(), "{}: input power {} after {ops:?}", project.antenna.name, r.total_power_w);
        }
        let layout = pulse_layout(&edited, &options()).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(layout.sources.iter().all(Option::is_some));
    }
}
