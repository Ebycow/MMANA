//! Checks for shapes MININEC calculates badly. The thresholds of the
//! same-wire shortcut come from measurements in `docs/measurements`, and
//! `mmana-measure` checks that the warning appears where the results move
//! and not where they do not. Overlapping wires are a matter of the shape
//! alone: conductors that pass through each other.

use crate::calc::{CalcWarning, WarningCode};
use crate::engine::Mininec;
use crate::engine::antdef::AntDef;

/// The kernel takes a same-wire shortcut for observation points of a joined
/// wire within `d0 + d3 <= 1.1` segment lengths of a source segment, meant
/// for points on the segment. A wire running beside it a few tenths of a
/// segment away (a folded dipole's end, the apex of a sharp V) meets that
/// test too and is calculated as if it lay on the segment (see
/// `Mininec::note_shortcut`). Measured in docs/measurements:
///
/// - It matters where the segment is at least this many times the distance:
///   the folded dipole breaks down from 4.36 times (where `d0 + d3` falls to
///   1.1 for a point beside the middle), a feed wire meeting a phasing line
///   at 34° is taken at 2.1 times and holds (HB9CVH, samples.md).
pub const SHORTCUT_RATIO: f64 = 4.0;

/// - ...and the segment is longer than this fraction of the wavelength (as a
///   divisor): the sharper V's move 1.1-2.1 % with λ/167 segments and at
///   most 0.36 % with λ/333 (v-segments.md); a 10° double dipole tapered to
///   λ/400 at its apex holds (DBLDP, samples.md).
pub const SHORTCUT_WAVELENGTHS: f64 = 250.0;

/// The worst place the matrix took the same-wire shortcut for a point
/// beside the source segment, as a warning.
pub(crate) fn misapplied_shortcut(ap: &AntDef, engine: &Mininec) -> Option<CalcWarning> {
    let s = (*engine.shortcut.lock().unwrap_or_else(|e| e.into_inner()))?;
    if s.segment * SHORTCUT_WAVELENGTHS <= engine.wavelength {
        return None;
    }
    let base = ap.base_wires.max(1);
    let defined = |w: usize| ap.wz.get(w.wrapping_sub(1)).map_or(w, |z| z.wno % base + 1);
    let mut wires = vec![defined(s.obs_wire), defined(s.src_wire)];
    wires.sort_unstable();
    wires.dedup();
    let names = wires.iter().map(|w| w.to_string()).collect::<Vec<_>>().join(" and ");
    Some(CalcWarning::new(
        WarningCode::CloseWires,
        wires,
        format!(
            "wire {names}: a point {:.4} m beside a {:.3} m segment of a joined wire (the segment is {:.1} times \
             the distance) is calculated as if it lay on the segment; MININEC's results break down there (a \
             folded dipole's resonance moved by 6-16 %), so divide the wires there into segments shorter than \
             {:.4} m",
            s.distance,
            s.segment,
            s.segment / s.distance,
            (s.distance * SHORTCUT_RATIO).max(engine.wavelength / SHORTCUT_WAVELENGTHS)
        ),
    ))
}

/// Wires that run along each other within the sum of their radii, over a
/// length greater than that sum: the conductors pass through each other.
/// Two wires with the very same ends are refused before the calculation
/// (`validate`); these are calculated, to no purpose. A copy of a dipole a
/// rounding to a radius beside it gives 441 - j528 Ω in place of 78 - j5 Ω
/// with a gain that looks right; a piece laid along a part of it, anything
/// from 84 - j59 Ω to 90 + j2071 Ω; a side of a loop turned onto the next
/// side, a gain between -54 and -72 dBi by the rounding of its end. Wires
/// that meet end to end, at an angle or across each other share no length.
pub(crate) fn overlapping_wires(ap: &AntDef) -> Option<CalcWarning> {
    type P = [f64; 3];
    let sub = |a: P, b: P| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: P, b: P| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let at = |a: P, d: P, t: f64| [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t];
    // from `p` to the nearest point of the wire from `a` to `b`
    let apart = |p: P, a: P, b: P| {
        let d = sub(b, a);
        let l2 = dot(d, d);
        let t = if l2 > 0.0 { (dot(sub(p, a), d) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let v = sub(p, at(a, d, t));
        dot(v, v).sqrt()
    };
    let ends = |i: usize| {
        let w = &ap.wz[i];
        ([w.x1, w.y1, w.z1], [w.x2, w.y2, w.z2])
    };
    let mut worst: Option<(f64, f64, usize, usize)> = None;
    let mut wires = Vec::new();
    for i in 0..ap.wz.len() {
        let (a1, a2) = ends(i);
        let d = sub(a2, a1);
        let length = dot(d, d).sqrt();
        if ap.wz[i].r <= 0.0 || length <= 0.0 {
            continue;
        }
        for j in i + 1..ap.wz.len() {
            if ap.wz[j].r <= 0.0 {
                continue;
            }
            let contact = ap.wz[i].r + ap.wz[j].r;
            // the part of wire i that wire j runs beside
            let (b1, b2) = ends(j);
            let (t1, t2) = (dot(sub(b1, a1), d) / length, dot(sub(b2, a1), d) / length);
            let (from, to) = (t1.min(t2).max(0.0), t1.max(t2).min(length));
            let shared = to - from;
            if shared <= contact {
                continue;
            }
            // straight wires: as close as this at both ends of the part, so all along it
            let gap = apart(at(a1, d, from / length), b1, b2).max(apart(at(a1, d, to / length), b1, b2));
            if gap <= contact {
                wires.extend([ap.wz[i].wno, ap.wz[j].wno]);
                if worst.is_none_or(|w| shared > w.0) {
                    worst = Some((shared, gap, ap.wz[i].wno, ap.wz[j].wno));
                }
            }
        }
    }
    let (shared, gap, first, second) = worst?;
    let base = ap.base_wires.max(1);
    let mut wires: Vec<usize> = wires.into_iter().map(|w| w % base + 1).collect();
    wires.sort_unstable();
    wires.dedup();
    Some(CalcWarning::new(
        WarningCode::OverlappingWires,
        wires,
        format!(
            "wires {} and {} run along each other for {shared:.4} m, {gap:.2e} m apart, which is within the sum of \
             their radii: the conductors pass through each other, which MININEC cannot calculate (the result \
             depends on the segments and on rounding), so remove one of them or move them apart",
            first % base + 1,
            second % base + 1
        ),
    ))
}
