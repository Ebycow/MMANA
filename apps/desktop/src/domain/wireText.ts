// Wires as text, one `.maa` wire line each (metres: X1, Y1, Z1, X2, Y2, Z2,
// R, SEG), so they can travel through the system clipboard between windows
// and through text editors and spreadsheets. Mirrors the wire section of
// `maa.rs` in mmana-core; notes and groups are not part of a wire line.
import type { Wire } from '../api/types';
import { strDbl } from './elements';
import { formatSegmentation, segmentationFromCode } from './segmentation';

/** `StrDblE`: like `strDbl`, but values below 1e-3 use `%.3le` (`8.000e-04`). */
function strDblE(d: number): string {
  if (d === 0 || Math.abs(d) >= 1e-3) return strDbl(d);
  const [mantissa, exp] = d.toExponential(3).split('e');
  const e = Number(exp);
  return `${mantissa}e${e < 0 ? '-' : '+'}${String(Math.abs(e)).padStart(2, '0')}`;
}

export function wiresToText(wires: Wire[]): string {
  return wires
    .map((w) =>
      [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z, w.radius].map(strDblE).join(',\t') +
      `,\t${formatSegmentation(w.segmentation)}\r\n`,
    )
    .join('');
}

/**
 * The wires in `text`, or null unless every non-blank line is a wire line.
 * Fields may be separated by commas, tabs or spaces; a missing SEG is 0.
 */
export function wiresFromText(text: string): Wire[] | null {
  const lines = text.split(/\r?\n/).filter((l) => l.trim() !== '');
  if (lines.length === 0) return null;
  const wires: Wire[] = [];
  for (const line of lines) {
    const f = line.trim().split(/[\s,]+/).filter((s) => s !== '');
    if (f.length < 7 || f.length > 9) return null;
    const n = f.map(Number);
    if (n.some((v) => !Number.isFinite(v)) || !n.slice(7).every(Number.isInteger)) return null;
    let seg = n[7] ?? 0;
    // "dm1,dm2" in the SEG column, as the .maa reader takes it
    if (n[8]) seg = (0x80000000 | Math.abs(seg) | (Math.abs(n[8]) << 16)) | 0;
    wires.push({
      start: { x: n[0], y: n[1], z: n[2] },
      end: { x: n[3], y: n[4], z: n[5] },
      radius: n[6],
      segmentation: segmentationFromCode(seg),
    });
  }
  return wires;
}
