// The printed sheets (印刷): which sections there are, how they fall onto A4
// pages, and the figures MMANA printed that the screen does not show (the
// SWR against 600 Ω, the SWR bandwidths of the 周波数特性 sheet).
import type { Complex, SweepPoint, Wire } from '../api/types';

export type SectionId = 'summary' | 'patterns' | 'shape' | 'warnings' | 'elements' | 'wires' | 'feed' | 'sweep' | 'memo';

export interface Section {
  id: SectionId;
  label: string;
  description: string;
  /** Printed unless the user leaves it out. */
  initial: boolean;
}

export const SECTIONS: Section[] = [
  { id: 'summary', label: '計算結果', description: '利得・F/B・Z・SWR・打ち上げ角と計算条件（MMANA のパターン図の結果欄を含む）', initial: true },
  { id: 'patterns', label: '指向性図', description: '水平面と垂直面', initial: true },
  { id: 'shape', label: 'アンテナ形状', description: '上・前・横から見た 3 面図と全体の寸法', initial: true },
  { id: 'warnings', label: '警告', description: '結果を信用できない可能性のある箇所', initial: true },
  { id: 'elements', label: 'エレメント表', description: 'エレメント編集と同じ表（位置・間隔・横幅・縦幅・全周）', initial: true },
  { id: 'wires', label: 'ワイヤー表', description: '全ワイヤーの座標・半径・分割数・長さ', initial: true },
  { id: 'feed', label: '給電点・負荷', description: '給電点ごとのインピーダンス・SWR と負荷の値', initial: true },
  { id: 'sweep', label: '周波数特性', description: 'SWR・インピーダンス・利得・F/B のグラフと SWR の帯域幅', initial: true },
  { id: 'memo', label: 'メモ欄', description: '製作や調整の記録を書き込む罫線', initial: false },
];

/** A4 portrait and the frame of every sheet, in mm. */
export const SHEET = { width: 210, height: 297, margin: 12, header: 9, footer: 7 } as const;
/** The height left for sections on a sheet. */
export const BODY_HEIGHT = SHEET.height - SHEET.margin * 2 - SHEET.header - SHEET.footer;
export const SECTION_GAP = 5;
/** Table rows and heads, in mm (8 pt text). */
export const ROW_HEIGHT = 4.6;
export const TABLE_HEAD = 6;
/** A section's heading line. */
export const HEADING = 7;

/** A section of a fixed height, or a table that may run on over several sheets. */
export type PrintBlock = { key: string; height: number } | { key: string; head: number; rows: number };

/** A block on a sheet: its height there and, for a table, the rows it shows. */
export interface Placed {
  key: string;
  height: number;
  rows?: { start: number; end: number };
}

/** Fewest rows a table starts with at the foot of a sheet; fewer go to the next. */
const MIN_ROWS = 3;

const tableHeight = (head: number, rows: number) => head + rows * ROW_HEIGHT;

/**
 * Places the blocks in order, starting a sheet when the next one does not
 * fit; a table is cut between rows, repeating its head on the next sheet.
 */
export function paginate(blocks: PrintBlock[], body = BODY_HEIGHT, gap = SECTION_GAP): Placed[][] {
  const sheets: Placed[][] = [[]];
  let used = 0;
  const place = (p: Placed) => {
    sheets.at(-1)!.push(p);
    used += (used > 0 ? gap : 0) + p.height;
  };
  const room = () => body - used - (used > 0 ? gap : 0);
  const newSheet = () => {
    if (sheets.at(-1)!.length > 0) sheets.push([]);
    used = 0;
  };

  for (const b of blocks) {
    if (!('rows' in b)) {
      if (b.height > room()) newSheet();
      place({ key: b.key, height: b.height });
      continue;
    }
    let start = 0;
    while (start < b.rows) {
      const fit = Math.floor((room() - b.head) / ROW_HEIGHT);
      const left = b.rows - start;
      if (fit < Math.min(MIN_ROWS, left)) {
        // a fresh sheet always takes at least one row
        if (used === 0) throw new Error('a sheet cannot hold a table row');
        newSheet();
        continue;
      }
      const end = start + Math.min(fit, left);
      place({ key: b.key, height: tableHeight(b.head, end - start), rows: { start, end } });
      start = end;
    }
  }
  return sheets.filter((s) => s.length > 0);
}

/** SWR of an impedance against a resistive reference (MMANA's `CalcSWR`). */
export function swrAgainst(z: Complex, r0: number): number {
  const num = Math.hypot(z.re - r0, z.im);
  const den = Math.hypot(z.re + r0, z.im);
  const g = den === 0 ? 1 : num / den;
  return g >= 1 ? Infinity : (1 + g) / (1 - g);
}

export interface Bandwidth {
  lowMhz: number;
  highMhz: number;
  khz: number;
  /** The SWR stays under the limit at an end of the sweep, so the band is wider than this. */
  open: boolean;
}

/**
 * The band around the lowest SWR where the SWR stays under `limit` (MMANA's
 * `GetSWRWidth`), with its edges interpolated between sweep points; null
 * when no point is under it.
 */
export function swrBandwidth(points: SweepPoint[], limit: number): Bandwidth | null {
  if (points.length === 0) return null;
  let best = 0;
  points.forEach((p, i) => p.swr < points[best].swr && (best = i));
  if (!(points[best].swr < limit)) return null;
  const cross = (i: number, j: number) => {
    const a = points[i];
    const b = points[j];
    const t = (limit - a.swr) / (b.swr - a.swr);
    return a.frequencyMhz + t * (b.frequencyMhz - a.frequencyMhz);
  };
  let lo = best;
  while (lo > 0 && points[lo - 1].swr < limit) lo--;
  let hi = best;
  while (hi < points.length - 1 && points[hi + 1].swr < limit) hi++;
  const lowMhz = lo > 0 ? cross(lo, lo - 1) : points[0].frequencyMhz;
  const highMhz = hi < points.length - 1 ? cross(hi, hi + 1) : points[points.length - 1].frequencyMhz;
  return { lowMhz, highMhz, khz: (highMhz - lowMhz) * 1000, open: lo === 0 || hi === points.length - 1 };
}

export interface Extent {
  min: { x: number; y: number; z: number };
  max: { x: number; y: number; z: number };
}

/** The box around every wire, in metres; null for no wires. */
export function extentOf(wires: Wire[]): Extent | null {
  if (wires.length === 0) return null;
  const pts = wires.flatMap((w) => [w.start, w.end]);
  const axis = (k: 'x' | 'y' | 'z', f: (...v: number[]) => number) => f(...pts.map((p) => p[k]));
  return {
    min: { x: axis('x', Math.min), y: axis('y', Math.min), z: axis('z', Math.min) },
    max: { x: axis('x', Math.max), y: axis('y', Math.max), z: axis('z', Math.max) },
  };
}
