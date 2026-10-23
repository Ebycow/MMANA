//! Checks for shapes MININEC calculates badly. Each threshold here comes
//! from a measurement in `docs/measurements`, and `mmana-measure` checks that
//! the warning appears where the results move and not where they do not.

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
