//! Inputs MMANA's dialogs never produce but project JSON and API requests can.

use mmana_core::{CalcError, CalcOptions, Project, Segmentation, calculate, maa};
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
    let cases: [(&str, Edit); 7] = [
        ("sc = 1", |p| p.antenna.auto_segmentation.sc = 1.0),
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
fn accepts_settings_in_range() {
    let mut p = dp20();
    p.antenna.auto_segmentation.sc = 3.0;
    p.antenna.wires[0].segmentation = Segmentation::TaperCustom { dm1: 800, dm2: Some(20) };
    calc(p).unwrap();
}
