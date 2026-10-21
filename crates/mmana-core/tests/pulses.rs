//! Pulse layout: the pulses sources and loads can be placed on, by name.

use mmana_core::model::{GroundKind, Point3, Segmentation, StackDirection};
use mmana_core::{CalcOptions, Project, calculate, maa, pulse_layout};
use std::path::Path;

fn sample(name: &str) -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa").join(name);
    maa::parse_bytes(&std::fs::read(path).unwrap()).project
}

#[test]
fn names_resolve_to_the_pulses_the_calculation_uses() {
    for name in ["DP20.MAA", "3EL20.MAA", "2CQ20.MAA", "2DELTA20.MAA"] {
        let p = sample(name);
        let opts = CalcOptions::default();
        let layout = pulse_layout(&p, &opts).unwrap();
        let result = calculate(&p, &opts).unwrap();
        assert_eq!(layout.pulse_count, result.pulse_count, "{name}");
        for w in &layout.wires {
            assert!(!w.pulses.is_empty(), "{name} wire {}", w.wire);
            for pt in &w.pulses {
                // the same pulse in the calculation, at the same place
                let c = &result.currents[pt.pulse - 1];
                assert!((c.position.x - pt.position.x).abs() < 1e-9, "{name} {}", pt.name);
                assert!((c.position.y - pt.position.y).abs() < 1e-9, "{name} {}", pt.name);
                assert!((c.position.z - pt.position.z).abs() < 1e-9, "{name} {}", pt.name);
                // and the name a source placed there would be given
                let mut moved = p.clone();
                moved.antenna.sources[0].pulse = pt.name.clone();
                let again = pulse_layout(&moved, &opts).unwrap();
                assert_eq!(again.sources[0], Some(pt.position), "{name} {}", pt.name);
            }
        }
    }
}

#[test]
fn centre_and_ends_are_named_from_the_nearer_end() {
    let layout = pulse_layout(&sample("3EL20.MAA"), &CalcOptions::default()).unwrap();
    let names: Vec<&str> = layout.wires[0].pulses.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names.first(), Some(&"w1b"));
    assert_eq!(names.get(1), Some(&"w1b1"));
    assert_eq!(names.last(), Some(&"w1e1"));
    let centre = layout.wires[0].pulses.iter().find(|p| p.name == "w1c").unwrap();
    assert!(centre.position.y.abs() < 1e-9);
    // pulses run from the start of the wire to its end
    let ys: Vec<f64> = layout.wires[0].pulses.iter().map(|p| p.position.y).collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn a_joint_belongs_to_both_wires() {
    // 2DELTA20: wire 2 ends where wire 3 begins (the feed point)
    let layout = pulse_layout(&sample("2DELTA20.MAA"), &CalcOptions::default()).unwrap();
    let end2 = layout.wires[1].pulses.last().unwrap();
    let start3 = layout.wires[2].pulses.first().unwrap();
    assert_eq!((end2.name.as_str(), start3.name.as_str()), ("w2e", "w3b"));
    assert_eq!(end2.pulse, start3.pulse);
    assert_eq!(layout.sources, vec![Some(start3.position)]);
}

#[test]
fn names_the_first_stacked_copy_and_leaves_out_the_others() {
    let mut p = sample("DP20.MAA");
    let plain = pulse_layout(&p, &CalcOptions::default()).unwrap();
    p.antenna.stack.horizontal = 2;
    p.antenna.stack.horizontal_spacing = 5.0;
    let stacked = pulse_layout(&p, &CalcOptions::default()).unwrap();
    let names = |l: &mmana_core::PulseLayout| -> Vec<(usize, String)> {
        l.wires.iter().flat_map(|w| w.pulses.iter().map(|q| (q.pulse, q.name.clone()))).collect()
    };
    assert_eq!(names(&stacked), names(&plain));
    for (a, b) in stacked.wires[0].pulses.iter().zip(&plain.wires[0].pulses) {
        assert!((a.position.y - b.position.y).abs() < 1e-9, "{}", a.name);
    }
    // w2c is the second copy's wire 1, which no name of the antenna addresses
    p.antenna.sources[0].pulse = "w2c".into();
    assert_eq!(pulse_layout(&p, &CalcOptions::default()).unwrap().sources, vec![None]);
}

#[test]
fn a_stacked_copy_on_the_ground_is_divided_as_calculated() {
    // a 5 m vertical in 10, 5 m up, stacked 2 high 10 m apart: the lower copy
    // (the first) stands on the ground and gains a pulse there
    let mut p = sample("DP20.MAA");
    let mut w = p.antenna.wires[0].clone();
    w.start = Point3::new(0.0, 0.0, 0.0);
    w.end = Point3::new(0.0, 0.0, 5.0);
    w.segmentation = Segmentation::from_code(10);
    p.antenna.wires = vec![w];
    p.antenna.sources.truncate(1);
    p.antenna.sources[0].pulse = "w1b".into();
    p.antenna.loads.clear();
    p.environment.ground = GroundKind::Perfect;
    p.environment.height = 5.0;
    p.antenna.stack.vertical = 2;
    p.antenna.stack.vertical_spacing = 10.0;
    p.antenna.stack.direction = StackDirection::Centered;
    let opts = CalcOptions::default();
    let layout = pulse_layout(&p, &opts).unwrap();
    let result = calculate(&p, &opts).unwrap();
    assert_eq!(layout.pulse_count, result.pulse_count);
    assert_eq!(layout.wires[0].pulses.len(), 10);
    for pt in &layout.wires[0].pulses {
        let c = &result.currents[pt.pulse - 1];
        assert!((c.position.z - (pt.position.z - 5.0)).abs() < 1e-9, "{}", pt.name);
    }
    assert_eq!(layout.sources, vec![Some(Point3::new(0.0, 0.0, 0.0))]);
}

#[test]
fn follows_the_calculation_frequency() {
    let p = sample("DP20.MAA");
    let design = pulse_layout(&p, &CalcOptions::default()).unwrap();
    let higher =
        pulse_layout(&p, &CalcOptions { frequency_mhz: Some(p.antenna.frequency_mhz * 3.0), ..Default::default() })
            .unwrap();
    // automatic segmentation is finer at a shorter wavelength
    assert!(higher.pulse_count > design.pulse_count);
}
