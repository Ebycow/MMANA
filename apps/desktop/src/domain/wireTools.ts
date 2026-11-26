// Two wire-table tools of MMANA's 編集 menu that change every wire at once:
// アンテナのサイズ (拡大・縮小) (`TWireScaleDlg`, WireScl.cpp) and 検索と置換
// (`TValRepDlg`, ValRep.cpp).
import type { Antenna, Segmentation, Wire } from '../api/types';
import { lengthFromDisplay, lengthToDisplay, radiusFromDisplay, radiusToDisplay, strDbl, type Units } from './elements';
import { segmentationToCode } from './segmentation';

/** What アンテナのサイズ scales: the coordinates along each axis and the wire radius. */
export interface ScaleTargets {
  x: boolean;
  y: boolean;
  z: boolean;
  radius: boolean;
}

export const ALL_SCALE_TARGETS: ScaleTargets = { x: true, y: true, z: true, radius: true };

/**
 * The 換算倍率 for moving a design from `fromMhz` to `toMhz`
 * (`TWireScaleDlg::TFreqChange`: tsc = fq / tfq).
 */
export const scaleForFrequency = (fromMhz: number, toMhz: number) => fromMhz / toMhz;

/** The 目標周波数 a 換算倍率 gives (`TWireScaleDlg::TSCChange`: tfq = fq / tsc). */
export const frequencyForScale = (fromMhz: number, scale: number) => fromMhz / scale;

/**
 * `TWireScaleDlg::Execute`: multiplies the chosen coordinates of every wire,
 * about the origin, by `scale`, and the radius of wires with a positive one
 * (zero disables a wire, a negative one names a combined wire). When a
 * coordinate is scaled, the design frequency becomes `targetMhz`.
 *
 * Like the original it leaves the loads, the stack spacing, the height above
 * ground and the combined-wire definitions alone. Returns null when it would
 * change nothing (`scale` 1, or nothing chosen).
 */
export function scaleAntenna(antenna: Antenna, scale: number, targetMhz: number, t: ScaleTargets): Antenna | null {
  if (!(scale > 0) || !(targetMhz > 0) || scale === 1) return null;
  const coords = t.x || t.y || t.z;
  if (!coords && !t.radius) return null;
  const wires = antenna.wires.map((w): Wire => {
    const s = { ...w.start };
    const e = { ...w.end };
    if (t.x) {
      s.x *= scale;
      e.x *= scale;
    }
    if (t.y) {
      s.y *= scale;
      e.y *= scale;
    }
    if (t.z) {
      s.z *= scale;
      e.z *= scale;
    }
    const radius = t.radius && w.radius > 0 ? w.radius * scale : w.radius;
    return { ...w, start: s, end: e, radius };
  });
  return { ...antenna, wires, ...(coords && { frequencyMhz: targetMhz }) };
}

/** 検索する座標: every coordinate, or one axis. */
export type CoordinateScope = 'all' | 'x' | 'y' | 'z';

/** What 検索と置換 searches, after the column its dialog was opened on. */
export type ReplaceTarget =
  | { kind: 'coordinate'; scope: CoordinateScope; absolute: boolean; search: number; replacement: number }
  | { kind: 'radius'; search: number; replacement: number }
  | { kind: 'segmentation'; search: Segmentation; replacement: Segmentation };

export interface Replaced {
  wires: Wire[];
  /** Values replaced (coordinates, radii or SEG cells). */
  count: number;
}

const SCOPE_AXES: Record<CoordinateScope, ('x' | 'y' | 'z')[]> = { all: ['x', 'y', 'z'], x: ['x'], y: ['y'], z: ['z'] };

/** How a value is compared: as the table shows it, rounded like `StrDbl`. */
const shown = (v: number) => Number(strDbl(v));

/**
 * `TValRepDlg::Execute`: replaces `search` with `replacement` in every wire.
 * Values are in the units the wire table shows (`u`), and a value matches
 * when it is `search` as the table shows it, to six decimals.
 *
 * With `absolute` (絶対値を検索), a coordinate whose magnitude matches is
 * replaced too, keeping its sign relative to `search` (`RepJob`): searching 5
 * for 7 turns −5 into −7, and searching −5 for 7 turns 5 into −7.
 */
export function replaceInWires(wires: Wire[], target: ReplaceTarget, u: Units): Replaced {
  let count = 0;
  if (target.kind === 'segmentation') {
    const from = segmentationToCode(target.search);
    if (from === segmentationToCode(target.replacement)) return { wires, count };
    const out = wires.map((w) => {
      if (segmentationToCode(w.segmentation) !== from) return w;
      count++;
      return { ...w, segmentation: structuredClone(target.replacement) };
    });
    return { wires: out, count };
  }
  if (target.search === target.replacement) return { wires, count };
  if (target.kind === 'radius') {
    const value = radiusFromDisplay(target.replacement, u);
    const out = wires.map((w) => {
      if (shown(radiusToDisplay(w.radius, u)) !== target.search) return w;
      count++;
      return { ...w, radius: value };
    });
    return { wires: out, count };
  }
  const { search, replacement, absolute } = target;
  const axes = SCOPE_AXES[target.scope];
  const replaced = (m: number): number | null => {
    const t = shown(lengthToDisplay(m, u));
    if (t === search) return lengthFromDisplay(replacement, u);
    if (!absolute || Math.abs(t) !== Math.abs(search)) return null;
    if (search === 0) return lengthFromDisplay(replacement, u);
    return lengthFromDisplay(Math.sign(search) * Math.sign(replacement) * Math.sign(t) * Math.abs(replacement), u);
  };
  const out = wires.map((w) => {
    let changed = false;
    const point = (p: Wire['start']) => {
      const q = { ...p };
      for (const a of axes) {
        const v = replaced(p[a]);
        if (v == null) continue;
        q[a] = v;
        count++;
        changed = true;
      }
      return q;
    };
    const start = point(w.start);
    const end = point(w.end);
    return changed ? { ...w, start, end } : w;
  });
  return { wires: out, count };
}
