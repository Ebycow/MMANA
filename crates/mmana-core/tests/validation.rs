//! Inputs MMANA's dialogs never produce but project JSON and API requests can.

use mmana_core::{CalcError, CalcOptions, Point3, Project, Segmentation, calculate, maa};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

fn dp20() -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa/DP20.MAA");
    maa::parse_bytes(&std::fs::read(path).unwrap()).project
}

/// Calculates on another thread so a calculation that never ends fails the
/// test instead of hanging it.
fn calc(project: Project) -> Result<(), CalcError> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(calculate(&project, &CalcOptions::default()).map(|_| ()));
    });
    rx.recv_timeout(Duration::from_secs(30)).expect("calculation did not finish")
}

type Edit = fn(&mut Project);

#[test]
fn rejects_auto_segmentation_that_never_finishes() {
    let cases: [(&str, Edit); 9] = [
        ("sc = 1", |p| p.antenna.auto_segmentation.sc = 1.0),
        ("sc just above 1", |p| {
            p.antenna.auto_segmentation.sc = 1.000000000000001;
            p.antenna.wires[0].segmentation = Segmentation::from_code(-1);
        }),
        ("sc < 1.01", |p| p.antenna.auto_segmentation.sc = 1.009),
        ("sc < 1", |p| p.antenna.auto_segmentation.sc = 0.5),
        ("sc > 3", |p| p.antenna.auto_segmentation.sc = 3.5),
        ("sc NaN", |p| p.antenna.auto_segmentation.sc = f64::NAN),
        ("dm2 = 0", |p| p.antenna.auto_segmentation.dm2 = 0),
        ("dm1 < 0", |p| p.antenna.auto_segmentation.dm1 = -400),
        ("ec = 0", |p| p.antenna.auto_segmentation.ec = 0),
    ];
    for (name, apply) in cases {
        let mut p = dp20();
        apply(&mut p);
        assert!(matches!(calc(p), Err(CalcError::InvalidAutoSegmentation(_))), "{name}");
    }
}

#[test]
fn rejects_custom_taper_out_of_range() {
    for (dm1, dm2, max) in
        [(400, Some(0), 0x7fff), (0, Some(40), 0x7fff), (400, Some(0x8000), 0x7fff), (20000, None, 10000)]
    {
        let mut p = dp20();
        p.antenna.wires[0].segmentation = Segmentation::TaperCustom { dm1, dm2 };
        assert_eq!(calc(p), Err(CalcError::InvalidWireSegmentation { wire: 1, max }), "{dm1} {dm2:?}");
    }
}

#[test]
fn rejects_fixed_counts_below_one() {
    // 0 would run as auto, -1..-3 as tapering, and -2147483248 as DM1 = 400
    // with DM2 = 0, which never finishes
    for count in [0, -1, -5, -2147483248, i32::MIN] {
        let mut p = dp20();
        p.antenna.wires[0].segmentation = Segmentation::Fixed { count };
        assert_eq!(calc(p), Err(CalcError::InvalidSegmentCount { wire: 1, count }), "{count}");
    }
    // a disabled wire is not expanded, so its count does not matter
    let mut p = dp20();
    let mut off = p.antenna.wires[0].clone();
    off.radius = 0.0;
    off.segmentation = Segmentation::Fixed { count: -2147483248 };
    p.antenna.wires.push(off);
    calc(p).unwrap();
}

#[test]
fn accepts_settings_in_range() {
    let mut p = dp20();
    p.antenna.auto_segmentation.sc = 3.0;
    p.antenna.wires[0].segmentation = Segmentation::TaperCustom { dm1: 800, dm2: Some(20) };
    calc(p.clone()).unwrap();
    p.antenna.auto_segmentation.sc = 1.01;
    calc(p).unwrap();
}

/// `count` short single-segment wires away from the dipole.
fn filler_wires(p: &mut Project, count: usize) {
    let template = p.antenna.wires[0].clone();
    for k in 0..count {
        let mut w = template.clone();
        w.start = Point3 { x: 100.0, y: k as f64, z: 0.0 };
        w.end = Point3 { x: 100.0, y: k as f64, z: 0.1 };
        w.segmentation = Segmentation::Fixed { count: 1 };
        p.antenna.wires.insert(0, w);
    }
}

#[test]
fn rejects_expansion_that_crosses_the_wire_limit_part_way() {
    // the tapered dipole expands into several wires; only the first fits
    let mut p = dp20();
    p.antenna.wires[0].segmentation = Segmentation::Taper;
    filler_wires(&mut p, 4095);
    p.antenna.sources[0].pulse = "w4096c".into();
    assert_eq!(calc(p), Err(CalcError::TooManySegmentWires { max: 4096 }));
}

#[test]
fn rejects_wires_on_top_of_each_other() {
    // a copy left where it was made, in either direction; the original
    // carries on with NaN currents
    for reversed in [false, true] {
        let mut p = dp20();
        let mut copy = p.antenna.wires[0].clone();
        if reversed {
            std::mem::swap(&mut copy.start, &mut copy.end);
        }
        p.antenna.wires.push(copy);
        assert_eq!(calc(p), Err(CalcError::OverlappingWires { first: 1, second: 2 }), "reversed: {reversed}");
    }
}

#[test]
fn rejects_stacked_copies_on_top_of_each_other() {
    let mut p = dp20();
    p.antenna.stack.horizontal = 2;
    p.antenna.stack.horizontal_spacing = 0.0;
    assert_eq!(calc(p), Err(CalcError::NoFiniteSolution));
}

#[test]
fn accepts_wires_that_only_partly_overlap() {
    let mut p = dp20();
    let mut w = p.antenna.wires[0].clone();
    w.start = Point3::new(w.start.x + 0.001, w.start.y, w.start.z);
    w.end = Point3::new(w.end.x + 0.001, w.end.y, w.end.z);
    p.antenna.wires.push(w);
    assert_eq!(calc(p), Ok(()));
}

#[test]
fn accepts_a_disabled_copy_of_a_wire() {
    let mut p = dp20();
    let mut off = p.antenna.wires[0].clone();
    off.radius = 0.0;
    p.antenna.wires.push(off);
    assert_eq!(calc(p), Ok(()));
}
