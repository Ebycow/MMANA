//! The experiments: each varies one thing MININEC may model badly, over a
//! family of antennas resonant near 14 MHz in free space.

use mmana_core::{Point3, Project, Segmentation, Source, Wire};

/// The design frequency of every antenna here (MHz), and its wavelength (m).
pub const FREQUENCY: f64 = 14.05;
const WAVELENGTH: f64 = 299.8 / FREQUENCY;
/// Half the length of the dipoles: 0.48 λ in all.
const HALF: f64 = 0.24 * WAVELENGTH;

pub struct Case {
    /// The value of the parameter, as shown in the table.
    pub label: String,
    pub project: Project,
    /// The same antenna modelled in a way believed accurate, when there is one.
    pub reference: Option<Project>,
}

pub struct Experiment {
    pub id: &'static str,
    pub title: &'static str,
    /// The column heading of the parameter.
    pub parameter: &'static str,
    /// How the antennas are built, and what the reference is.
    pub description: &'static str,
    /// The cases; a few of them when `quick`.
    pub cases: fn(quick: bool) -> Vec<Case>,
}

pub const EXPERIMENTS: &[Experiment] = &[
    Experiment {
        id: "segments",
        title: "分割数（区間の長さと波長の比）",
        parameter: "分割数（区間の長さ）",
        description: "半波長ダイポール（0.48 λ、半径 1 mm）を等分する数を変える。基準は 200 分割（1 つの導体が 250 分割を超えると結果がずれるため、それより少ない数）。",
        cases: segments,
    },
    Experiment {
        id: "v-angle",
        title: "給電点での折れ角（V 型ダイポール）",
        parameter: "腕どうしの角度",
        description: "長さ 0.24 λ の腕 2 本を給電点で V 字につなぎ、腕どうしの角度を変える（180° はまっすぐなダイポール）。腕は各 20 分割、半径 1 mm。角度で共振は物理的に変わるので基準はなく、分割 2 倍での変化と AGT で見る。",
        cases: v_angle,
    },
    Experiment {
        id: "bend-angle",
        title: "腕の途中での折れ角",
        parameter: "折れ目の角度",
        description: "ダイポールの各腕を中ほどで折り曲げ、折れ目の内側の角度を変える（180° はまっすぐ、小さいほど折り返す）。各区間 10 分割、半径 1 mm。基準はなく、分割 2 倍での変化と AGT で見る。",
        cases: bend_angle,
    },
    Experiment {
        id: "radius-step",
        title: "接続部での半径の段差",
        parameter: "内側 ÷ 外側の半径",
        description: "ダイポールの各腕を内側・外側の 2 本に分け、内側を半径 10 mm、外側をその 1/k にする（八木のエレメントのテーパーに相当）。各 10 分割。基準はなく、分割 2 倍での変化と AGT で見る。",
        cases: radius_step,
    },
    Experiment {
        id: "folded-segments",
        title: "近接した平行ワイヤー：区間の長さと間隔の比（フォールデッドダイポール）",
        parameter: "間隔、区間の長さ ÷ 間隔（長い 2 本の分割数）",
        description: "長さ 0.48 λ、半径 1 mm の 2 本を平行に並べて両端をつなぎ、片方の中央で給電する（両端のワイヤーは 1 分割）。間隔を 10 cm・5 cm に固定し、長い 2 本の分割数を変える。基準は同じ間隔でいちばん細かく分けたもの（1 つの導体が 250 分割を超えないように、10 cm では 100 分割、5 cm では 120 分割）。",
        cases: folded_segments,
    },
    Experiment {
        id: "folded-radius",
        title: "近接した平行ワイヤー：間隔と半径の比（フォールデッドダイポール）",
        parameter: "間隔 ÷ 半径",
        description: "folded-segments と同じ形で、間隔 10 cm、長い 2 本を各 100 分割（区間 ≒ 間隔）にし、半径を変える。基準はなく、分割 2 倍での変化と AGT で見る。",
        cases: folded_radius,
    },
    Experiment {
        id: "segment-radius",
        title: "区間の長さと半径の比（太いワイヤー）",
        parameter: "区間の長さ ÷ 半径",
        description: "半波長ダイポール（0.48 λ、20 分割）の半径を変える。太さで共振は物理的に変わるので基準はなく、分割 2 倍での変化と AGT で見る。",
        cases: segment_radius,
    },
    Experiment {
        id: "segment-step",
        title: "接続部で隣り合う区間の長さの比",
        parameter: "内側の分割（片側）、外側 ÷ 内側の区間の長さ",
        description: "ダイポールの各腕を同じ長さの内側・外側の 2 本に分け、外側の区間を内側の k 倍の長さにする。内側が片側 10 分割（区間 λ/17）と 30 分割（λ/50）の 2 通り。基準は同じ形を全体で同じ長さの区間に分けたもの（外側も内側と同じ分割数）。",
        cases: segment_step,
    },
];

fn wire(a: (f64, f64, f64), b: (f64, f64, f64), radius: f64, count: i32) -> Wire {
    Wire {
        start: Point3::new(a.0, a.1, a.2),
        end: Point3::new(b.0, b.1, b.2),
        radius,
        segmentation: Segmentation::Fixed { count },
        note: String::new(),
    }
}

fn antenna(wires: Vec<Wire>, feed: &str) -> Project {
    let mut p = Project::default();
    p.antenna.name = "measure".into();
    p.antenna.frequency_mhz = FREQUENCY;
    p.antenna.wires = wires;
    p.antenna.sources = vec![Source { pulse: feed.into(), phase_deg: 0.0, voltage: 1.0 }];
    p
}

fn pick<T: Clone>(all: &[T], quick: bool) -> Vec<T> {
    if quick { vec![all[0].clone(), all[all.len() - 1].clone()] } else { all.to_vec() }
}

fn dipole(radius: f64, count: i32) -> Project {
    antenna(vec![wire((0.0, -HALF, 0.0), (0.0, HALF, 0.0), radius, count)], "w1c")
}

fn segments(quick: bool) -> Vec<Case> {
    let reference = dipole(0.001, 200);
    pick(&[2, 4, 6, 8, 10, 14, 20, 30, 50, 80], quick)
        .into_iter()
        .map(|n| Case {
            label: format!("{n}（λ/{:.0}）", WAVELENGTH / (2.0 * HALF / n as f64)),
            project: dipole(0.001, n),
            reference: Some(reference.clone()),
        })
        .collect()
}

/// Two arms from the origin, `deg` apart, in the Y-Z plane (180° is straight along Y).
fn v_dipole(deg: f64) -> Project {
    let h = (deg / 2.0).to_radians();
    let (y, z) = (HALF * h.sin(), -HALF * h.cos());
    antenna(vec![wire((0.0, 0.0, 0.0), (0.0, y, z), 0.001, 20), wire((0.0, 0.0, 0.0), (0.0, -y, z), 0.001, 20)], "w1b")
}

fn v_angle(quick: bool) -> Vec<Case> {
    pick(&[180.0, 150.0, 120.0, 90.0, 60.0, 45.0, 30.0, 20.0, 10.0], quick)
        .into_iter()
        .map(|deg| Case { label: format!("{deg}°"), project: v_dipole(deg), reference: None })
        .collect()
}

/// A dipole whose arms bend halfway, the inside of the bend `deg` (180° is straight).
fn bent_dipole(deg: f64) -> Project {
    let a = HALF / 2.0;
    let t = deg.to_radians();
    // from the bend, at `deg` to the wire back to the feed
    let (dy, dz) = (-a * t.cos(), -a * t.sin());
    antenna(
        vec![
            wire((0.0, -a, 0.0), (0.0, a, 0.0), 0.001, 20),
            wire((0.0, a, 0.0), (0.0, a + dy, dz), 0.001, 10),
            wire((0.0, -a, 0.0), (0.0, -a - dy, dz), 0.001, 10),
        ],
        "w1c",
    )
}

fn bend_angle(quick: bool) -> Vec<Case> {
    pick(&[180.0, 150.0, 120.0, 90.0, 60.0, 45.0, 30.0, 20.0, 10.0], quick)
        .into_iter()
        .map(|deg| Case { label: format!("{deg}°"), project: bent_dipole(deg), reference: None })
        .collect()
}

/// A dipole of an inner half `r` and outer halves `r / k` in radius.
fn stepped_dipole(k: f64) -> Project {
    let a = HALF / 2.0;
    let r = 0.01;
    antenna(
        vec![
            wire((0.0, -a, 0.0), (0.0, a, 0.0), r, 20),
            wire((0.0, a, 0.0), (0.0, HALF, 0.0), r / k, 10),
            wire((0.0, -a, 0.0), (0.0, -HALF, 0.0), r / k, 10),
        ],
        "w1c",
    )
}

fn radius_step(quick: bool) -> Vec<Case> {
    pick(&[1.0, 1.5, 2.0, 3.0, 5.0, 10.0, 20.0], quick)
        .into_iter()
        .map(|k| Case { label: format!("{k}"), project: stepped_dipole(k), reference: None })
        .collect()
}

/// A folded dipole `s` m between the wire centres, of radius `r`, the long wires in `n` segments each.
fn folded(s: f64, r: f64, n: i32) -> Project {
    antenna(
        vec![
            wire((0.0, -HALF, 0.0), (0.0, HALF, 0.0), r, n),
            wire((0.0, HALF, 0.0), (0.0, HALF, s), r, 1),
            wire((0.0, HALF, s), (0.0, -HALF, s), r, n),
            wire((0.0, -HALF, s), (0.0, -HALF, 0.0), r, 1),
        ],
        "w1c",
    )
}

fn folded_segments(quick: bool) -> Vec<Case> {
    let mut cases = Vec::new();
    for (s, finest) in [(0.1, 100), (0.05, 120)] {
        let reference = folded(s, 0.001, finest);
        for ratio in pick(&[10.0, 8.0, 6.0, 5.0, 4.6, 4.3, 4.0, 3.0, 2.0, 1.5], quick) {
            // an even count, so the feed is at the centre
            let n = ((2.0 * HALF / (ratio * s) / 2.0).round() as i32 * 2).min(finest);
            let actual = 2.0 * HALF / n as f64 / s;
            if n == finest && actual < ratio * 0.9 {
                continue;
            }
            cases.push(Case {
                label: format!("{} cm、{actual:.1}（{n}）", s * 100.0),
                project: folded(s, 0.001, n),
                reference: Some(reference.clone()),
            });
        }
    }
    cases
}

fn folded_radius(quick: bool) -> Vec<Case> {
    let s = 0.1;
    pick(&[100.0, 50.0, 20.0, 10.0, 5.0, 3.0, 2.5], quick)
        .into_iter()
        .map(|k| Case { label: format!("{k}"), project: folded(s, s / k, 100), reference: None })
        .collect()
}

fn segment_radius(quick: bool) -> Vec<Case> {
    let segment = 2.0 * HALF / 20.0;
    pick(&[200.0, 50.0, 20.0, 10.0, 5.0, 3.0, 2.0, 1.0], quick)
        .into_iter()
        .map(|k| Case { label: format!("{k}"), project: dipole(segment / k, 20), reference: None })
        .collect()
}

/// A dipole of an inner half in `inner` segments a side and outer halves in `n` each.
fn split_dipole(inner: i32, n: i32) -> Project {
    let a = HALF / 2.0;
    antenna(
        vec![
            wire((0.0, -a, 0.0), (0.0, a, 0.0), 0.001, 2 * inner),
            wire((0.0, a, 0.0), (0.0, HALF, 0.0), 0.001, n),
            wire((0.0, -a, 0.0), (0.0, -HALF, 0.0), 0.001, n),
        ],
        "w1c",
    )
}

fn segment_step(quick: bool) -> Vec<Case> {
    let mut cases = Vec::new();
    for (inner, counts) in [(10, &[10, 5, 3, 2, 1][..]), (30, &[30, 20, 15, 10, 6, 5, 3, 2, 1][..])] {
        for n in pick(counts, quick) {
            cases.push(Case {
                label: format!("{inner}、{:.1}", inner as f64 / n as f64),
                project: split_dipole(inner, n),
                reference: Some(split_dipole(inner, inner)),
            });
        }
    }
    cases
}
