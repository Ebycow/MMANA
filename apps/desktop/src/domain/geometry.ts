// Geometry helpers for the 3D view: stack expansion (mirrors AntDef::stack in
// mmana-core), pulse-name positions, bounds and current-colouring polylines.
import type { Antenna, CalcResult, Point3, Wire } from '../api/types';

export interface DisplayWire {
  /** 0-based index into antenna.wires this copy comes from. */
  base: number;
  start: Point3;
  end: Point3;
  radius: number;
}

export function displayWires(ant: Antenna): DisplayWire[] {
  const base = ant.wires.map((w, i) => ({ base: i, start: w.start, end: w.end, radius: w.radius }));
  const st = ant.stack;
  let out = base;
  if (st.horizontal > 1) {
    const copies: DisplayWire[] = [];
    let yc = -((st.horizontal - 1) * st.horizontalSpacing) / 2;
    for (let i = 0; i < st.horizontal; i++, yc += st.horizontalSpacing) {
      for (const w of out) copies.push({ ...w, start: { ...w.start, y: w.start.y + yc }, end: { ...w.end, y: w.end.y + yc } });
    }
    out = copies;
  }
  if (st.vertical > 1) {
    const copies: DisplayWire[] = [];
    let zc =
      st.direction === 'up'
        ? 0
        : st.direction === 'down'
          ? -((st.vertical - 1) * st.verticalSpacing)
          : -((st.vertical - 1) * st.verticalSpacing) / 2;
    for (let i = 0; i < st.vertical; i++, zc += st.verticalSpacing) {
      for (const w of out) copies.push({ ...w, start: { ...w.start, z: w.start.z + zc }, end: { ...w.end, z: w.end.z + zc } });
    }
    out = copies;
  }
  return out;
}

const lerp = (a: Point3, b: Point3, t: number): Point3 => ({
  x: a.x + (b.x - a.x) * t,
  y: a.y + (b.y - a.y) * t,
  z: a.z + (b.z - a.z) * t,
});

/** Approximate location of a pulse name (`w2c`, `w2b`, `w2e`) for markers. */
/** Full-width ASCII and the ideographic space to half-width, as MMANA reads pulse names. */
export const toHalfWidth = (s: string) =>
  s.replace(/[\uff01-\uff5e]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/\u3000/g, ' ');

/** A pulse name as typed, in the form the engine reads it. */
export const normalizePulse = (s: string) => toHalfWidth(s).trim();

export function pulseLocation(name: string, wires: Wire[]): Point3 | null {
  const m = /^w(\d+)([bce]?)/i.exec(normalizePulse(name));
  if (!m) return null;
  const w = wires[Number(m[1]) - 1];
  if (!w) return null;
  const where = m[2].toLowerCase();
  return lerp(w.start, w.end, where === 'b' ? 0.04 : where === 'e' ? 0.96 : 0.5);
}

export interface Bounds {
  center: Point3;
  radius: number;
  minZ: number;
  maxZ: number;
}

export function bounds(wires: DisplayWire[]): Bounds {
  if (wires.length === 0) return { center: { x: 0, y: 0, z: 0 }, radius: 5, minZ: 0, maxZ: 0 };
  const lo = { x: Infinity, y: Infinity, z: Infinity };
  const hi = { x: -Infinity, y: -Infinity, z: -Infinity };
  for (const w of wires) {
    for (const p of [w.start, w.end]) {
      lo.x = Math.min(lo.x, p.x);
      lo.y = Math.min(lo.y, p.y);
      lo.z = Math.min(lo.z, p.z);
      hi.x = Math.max(hi.x, p.x);
      hi.y = Math.max(hi.y, p.y);
      hi.z = Math.max(hi.z, p.z);
    }
  }
  const center = { x: (lo.x + hi.x) / 2, y: (lo.y + hi.y) / 2, z: (lo.z + hi.z) / 2 };
  const radius = Math.max(0.5, Math.hypot(hi.x - lo.x, hi.y - lo.y, hi.z - lo.z) / 2);
  return { center, radius, minZ: lo.z, maxZ: hi.z };
}

/** The horizontal grid of the 3D view: a square of `half` either side of `center`, at height `z`. */
export interface SceneGrid {
  center: { x: number; y: number };
  z: number;
  half: number;
  /** Line spacing; every tenth line is a section line. */
  cell: number;
  section: number;
}

/**
 * Lays out the horizontal grid at height `z` (the ground, or Z = 0 in free
 * space). Lines fall on round coordinates in the snap step, or a power of ten
 * times it so a large scene does not turn into a haze, and the square covers
 * the antenna's footprint and enough ground around it to read its height. The
 * layout only changes when the antenna grows past a section or moves by one.
 */
export function sceneGrid(box: Bounds, z: number, step: number): SceneGrid {
  const want = Math.max(box.radius * 2, (box.maxZ - z) * 0.6);
  let cell = step;
  while ((2 * want) / cell > 100) cell = Number((cell * 10).toPrecision(6));
  const section = Number((cell * 10).toPrecision(6));
  const half = Math.max(1, Math.ceil(want / section - 1e-9)) * section;
  const snap = (v: number) => Number((Math.round(v / section) * section).toPrecision(12)) + 0;
  return { center: { x: snap(box.center.x), y: snap(box.center.y) }, z, half, cell, section };
}

export interface CurrentPolyline {
  /** Index into displayWires(). */
  wire: number;
  points: Point3[];
  /** Current magnitude at each point, relative to the maximum (0..1). */
  levels: number[];
}

/**
 * Builds one polyline per wire through its pulse positions, so the wire can be
 * coloured by current magnitude. Pulses report the (stacked) defining wire.
 */
export function currentPolylines(wires: DisplayWire[], result: CalcResult): { lines: CurrentPolyline[]; maxMa: number } {
  const mags = result.currents.map((c) => Math.hypot(c.current.re, c.current.im));
  const max = Math.max(1e-30, ...mags);
  const byWire = new Map<number, { t: number; p: Point3; m: number }[]>();
  result.currents.forEach((c, i) => {
    const w = wires[c.wire - 1];
    if (!w) return;
    const d = { x: w.end.x - w.start.x, y: w.end.y - w.start.y, z: w.end.z - w.start.z };
    const len2 = d.x * d.x + d.y * d.y + d.z * d.z || 1;
    const t = ((c.position.x - w.start.x) * d.x + (c.position.y - w.start.y) * d.y + (c.position.z - w.start.z) * d.z) / len2;
    const list = byWire.get(c.wire - 1) ?? [];
    list.push({ t: Math.min(1, Math.max(0, t)), p: c.position, m: mags[i] / max });
    byWire.set(c.wire - 1, list);
  });
  const lines: CurrentPolyline[] = [];
  byWire.forEach((pts, wire) => {
    const w = wires[wire];
    pts.sort((a, b) => a.t - b.t);
    const points = [w.start, ...pts.map((q) => q.p), w.end];
    const levels = [pts[0].m, ...pts.map((q) => q.m), pts[pts.length - 1].m];
    lines.push({ wire, points, levels });
  });
  return { lines, maxMa: max * 1000 };
}
