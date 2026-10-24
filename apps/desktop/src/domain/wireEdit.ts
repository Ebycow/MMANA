// Direct editing of wires in the 3D view: transforms of a selection, mirror,
// copies, deletion with pulse renumbering, shapes drawn on a plane and the
// snapping helpers. Pure functions; the view keeps the pointer state.
import type { Antenna, Point3, Wire } from '../api/types';
import { normalizePulse } from './geometry';
import { firstWire, setAutoVoltage } from './project';

export type Axis = 'x' | 'y' | 'z';
export const AXES: Axis[] = ['x', 'y', 'z'];

/** A drawing plane through the origin, named by the two axes it contains. */
export type Plane = 'xy' | 'xz' | 'yz';
export const PLANE_NORMAL: Record<Plane, Axis> = { xy: 'z', xz: 'y', yz: 'x' };
export const PLANE_AXES: Record<Plane, [Axis, Axis]> = { xy: ['x', 'y'], xz: ['x', 'z'], yz: ['y', 'z'] };

/**
 * The plane that faces a camera looking along `dir` (a unit vector): the
 * ground plane (XY) unless the camera looks within about 20° of horizontal,
 * then the vertical plane it looks most squarely at. Near a boundary the
 * `current` plane is kept, so a slight turn of the view does not flip it.
 */
export function facingPlane(dir: Point3, current: Plane | null): Plane {
  const down = Math.abs(dir.z);
  if (current === 'xy' ? down > 0.28 : current ? down > 0.42 : down > 0.35) return 'xy';
  const ax = Math.abs(dir.x);
  const ay = Math.abs(dir.y);
  if (current === 'xz' && ay * 1.3 >= ax) return 'xz';
  if (current === 'yz' && ax * 1.3 >= ay) return 'yz';
  return ay >= ax ? 'xz' : 'yz';
}

export const add = (a: Point3, b: Point3): Point3 => ({ x: a.x + b.x, y: a.y + b.y, z: a.z + b.z });
export const sub = (a: Point3, b: Point3): Point3 => ({ x: a.x - b.x, y: a.y - b.y, z: a.z - b.z });
export const scale = (a: Point3, k: number): Point3 => ({ x: a.x * k, y: a.y * k, z: a.z * k });
export const dot = (a: Point3, b: Point3) => a.x * b.x + a.y * b.y + a.z * b.z;
export const cross = (a: Point3, b: Point3): Point3 => ({ x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x });
export const norm = (a: Point3) => Math.hypot(a.x, a.y, a.z);
export const distance = (a: Point3, b: Point3) => norm(sub(a, b));

/** Endpoints closer than this are the same point (a junction). */
export const JOIN_EPS = 1e-6;
export const samePoint = (a: Point3, b: Point3) => distance(a, b) < JOIN_EPS;

/** Rounds to 1 nm, so that moved and rotated points do not carry 1e-17 noise. */
const clean = (v: number) => Math.round(v * 1e9) / 1e9 + 0;
export const cleanPoint = (p: Point3): Point3 => ({ x: clean(p.x), y: clean(p.y), z: clean(p.z) });

export type PointMap = (p: Point3) => Point3;

export const translation =
  (d: Point3): PointMap =>
  (p) =>
    add(p, d);

/** Rotation by `deg` about the line through `pivot` along `axis` (right-handed). */
export function rotation(pivot: Point3, axis: Point3, deg: number): PointMap {
  const n = scale(axis, 1 / (norm(axis) || 1));
  const a = (deg * Math.PI) / 180;
  const c = Math.cos(a);
  const s = Math.sin(a);
  return (p) => {
    // Rodrigues: v cos + (n × v) sin + n (n · v)(1 − cos)
    const v = sub(p, pivot);
    const r = add(add(scale(v, c), scale(cross(n, v), s)), scale(n, dot(n, v) * (1 - c)));
    return add(pivot, r);
  };
}

export const unitAxis = (axis: Axis): Point3 => ({ x: axis === 'x' ? 1 : 0, y: axis === 'y' ? 1 : 0, z: axis === 'z' ? 1 : 0 });

/** Centre of the bounding box of the selected wires, or null for an empty selection. */
export function selectionCenter(wires: Wire[], selection: number[]): Point3 | null {
  const lo = { x: Infinity, y: Infinity, z: Infinity };
  const hi = { x: -Infinity, y: -Infinity, z: -Infinity };
  let any = false;
  for (const i of selection) {
    const w = wires[i];
    if (!w) continue;
    any = true;
    for (const p of [w.start, w.end]) {
      for (const k of AXES) {
        lo[k] = Math.min(lo[k], p[k]);
        hi[k] = Math.max(hi[k], p[k]);
      }
    }
  }
  return any ? { x: (lo.x + hi.x) / 2, y: (lo.y + hi.y) / 2, z: (lo.z + hi.z) / 2 } : null;
}

/** Two wires with the same ends, in either direction. */
const sameWire = (a: Wire, b: Wire) =>
  (samePoint(a.start, b.start) && samePoint(a.end, b.end)) || (samePoint(a.start, b.end) && samePoint(a.end, b.start));

/**
 * Wires outside the selection that lie exactly on a selected wire, such as
 * the originals of a copy made in place. They overlap the selection rather
 * than join it, so they never follow it.
 */
function overlapping(wires: Wire[], sel: Set<number>): Set<number> {
  const out = new Set<number>();
  wires.forEach((w, i) => {
    if (!sel.has(i) && [...sel].some((j) => wires[j] && sameWire(w, wires[j]))) out.add(i);
  });
  return out;
}

/**
 * Applies `f` to both ends of the selected wires. With `follow`, ends of the
 * other wires that meet a moved end move with it, so junctions stay closed.
 */
export function transformWires(wires: Wire[], selection: number[], f: PointMap, follow: boolean): Wire[] {
  const sel = new Set(selection);
  const moved: Point3[] = [];
  for (const i of sel) if (wires[i]) moved.push(wires[i].start, wires[i].end);
  const meets = (p: Point3) => follow && moved.some((q) => samePoint(p, q));
  const skip = follow ? overlapping(wires, sel) : new Set<number>();
  return wires.map((w, i) => {
    if (sel.has(i)) return { ...w, start: cleanPoint(f(w.start)), end: cleanPoint(f(w.end)) };
    if (skip.has(i)) return w;
    const s = meets(w.start);
    const e = meets(w.end);
    if (!s && !e) return w;
    return { ...w, start: s ? cleanPoint(f(w.start)) : w.start, end: e ? cleanPoint(f(w.end)) : w.end };
  });
}

export type End = 'start' | 'end';

/**
 * Moves one end of a wire; with `follow`, the ends that meet it move too,
 * except those of wires lying on a wire of `selection` (the originals of a
 * copy made in place).
 */
export function moveEndpoint(wires: Wire[], wire: number, end: End, to: Point3, follow: boolean, selection: number[] = [wire]): Wire[] {
  const from = wires[wire]?.[end];
  if (!from) return wires;
  const p = cleanPoint(to);
  const skip = follow ? overlapping(wires, new Set([wire, ...selection])) : new Set<number>();
  return wires.map((w, i) => {
    if (skip.has(i)) return w;
    const s = i === wire ? end === 'start' : follow && samePoint(w.start, from);
    const e = i === wire ? end === 'end' : follow && samePoint(w.end, from);
    if (!s && !e) return w;
    return { ...w, start: s ? p : w.start, end: e ? p : w.end };
  });
}

export interface Edited {
  wires: Wire[];
  /** The selection after the edit (the new wires when copies were made). */
  selection: number[];
}

/**
 * Appends `count` copies of the selected wires, copy k (1-based) mapped by
 * `f(p, k)`. A copy that would lie exactly on an existing wire is skipped
 * unless `skipExisting` is false, so mirroring a wire that lies on the
 * mirror plane does not double it.
 */
export function appendCopies(
  wires: Wire[],
  selection: number[],
  count: number,
  f: (p: Point3, k: number) => Point3,
  skipExisting = true,
): Edited {
  const out = [...wires];
  const added: number[] = [];
  const src = [...selection].sort((a, b) => a - b).filter((i) => wires[i]);
  for (let k = 1; k <= count; k++) {
    for (const i of src) {
      const w = wires[i];
      const copy = { ...structuredClone(w), start: cleanPoint(f(w.start, k)), end: cleanPoint(f(w.end, k)) };
      if (skipExisting && out.some((o) => sameWire(o, copy))) continue;
      added.push(out.length);
      out.push(copy);
    }
  }
  return { wires: out, selection: added };
}

/** Reflection in the plane `axis` = `at`. */
export const reflection =
  (axis: Axis, at: number): PointMap =>
  (p) => ({ ...p, [axis]: 2 * at - p[axis] });

/** Flips the selection about its own centre (MMANA's 反転). */
export function mirrorInPlace(wires: Wire[], selection: number[], axis: Axis): Wire[] {
  const c = selectionCenter(wires, selection);
  return c ? transformWires(wires, selection, reflection(axis, c[axis]), false) : wires;
}

/** Adds the mirror image of the selection in the plane `axis` = 0 (build half, mirror the rest). */
export function mirrorCopy(wires: Wire[], selection: number[], axis: Axis): Edited {
  return appendCopies(wires, selection, 1, reflection(axis, 0));
}

/** Moves the selection so its centre is at 0 on `axis` (MMANA's 原点に揃える). */
export function alignToOrigin(wires: Wire[], selection: number[], axis: Axis, follow: boolean): Wire[] {
  const c = selectionCenter(wires, selection);
  if (!c || c[axis] === 0) return wires;
  return transformWires(wires, selection, translation({ x: 0, y: 0, z: 0, [axis]: -c[axis] }), follow);
}

/** Turns the selection by `deg` about `axis` through its centre. */
export function rotateSelection(wires: Wire[], selection: number[], axis: Axis, deg: number, follow: boolean): Wire[] {
  const c = selectionCenter(wires, selection);
  return c ? transformWires(wires, selection, rotation(c, unitAxis(axis), deg), follow) : wires;
}

/**
 * The new number of a wire in a pulse name (`w3c` → `w2c`), as mmana-core's
 * `follow_wire` does for the element editor. `moved[i]` is the new 0-based
 * index of wire i, or null when it was deleted.
 */
export function followWire(pulse: string, moved: (number | null)[]): { pulse: string; keep: boolean; warning?: string } {
  const name = normalizePulse(pulse);
  const m = /^([wW])(\d+)(.*)$/.exec(name);
  if (!m) return { pulse, keep: true, warning: `「${pulse}」はパルス番号なので、別のワイヤーを指すようになった可能性があります` };
  const n = Number(m[2]);
  if (n === 0 || n > moved.length) return { pulse, keep: true };
  const to = moved[n - 1];
  if (to == null) return { pulse, keep: false, warning: `「${pulse}」はワイヤーと一緒に削除しました` };
  return { pulse: `${m[1]}${to + 1}${m[3]}`, keep: true };
}

export type FollowResult = ReturnType<typeof followWire>;

/**
 * Replaces the wires and renames the pulses of sources and loads with
 * `rename`; those it does not keep are removed. Returns notes for the user.
 */
export function renumberWires(antenna: Antenna, wires: Wire[], rename: (pulse: string) => FollowResult): { antenna: Antenna; warnings: string[] } {
  const warnings: string[] = [];
  const follow = <T extends { pulse: string }>(items: T[], what: string) =>
    items.flatMap((item) => {
      const r = rename(item.pulse);
      if (r.warning) warnings.push(`${what}: ${r.warning}`);
      return r.keep ? [{ ...item, pulse: r.pulse }] : [];
    });
  const out: Antenna = { ...antenna, wires, sources: follow(antenna.sources, '給電点'), loads: follow(antenna.loads, '負荷') };
  if (out.sources.length !== antenna.sources.length) setAutoVoltage(out);
  return { antenna: out, warnings };
}

/** Deletes wires, renumbering sources and loads and removing those on a deleted wire. */
export function removeWires(antenna: Antenna, selection: number[]): { antenna: Antenna; warnings: string[] } {
  const del = new Set(selection);
  const moved: (number | null)[] = [];
  let next = 0;
  for (let i = 0; i < antenna.wires.length; i++) moved.push(del.has(i) ? null : next++);
  return renumberWires(
    antenna,
    antenna.wires.filter((_, i) => !del.has(i)),
    (p) => followWire(p, moved),
  );
}

/** Inserts wires before index `at`, renumbering the sources and loads of the wires after it. */
export function insertWires(antenna: Antenna, at: number, wires: Wire[]): { antenna: Antenna; warnings: string[] } {
  const moved = antenna.wires.map((_, i) => (i < at ? i : i + wires.length));
  return renumberWires(antenna, [...antenna.wires.slice(0, at), ...wires, ...antenna.wires.slice(at)], (p) => followWire(p, moved));
}

/** A reordering of the wires: the antenna, `moved[i]` = the new index of wire i, and where the moved wires went. */
export interface Moved {
  antenna: Antenna;
  warnings: string[];
  moved: number[];
  selection: number[];
}

/**
 * Moves the selected wires, in their order, into the gap before wire `at`
 * (the wire count for the end), renumbering sources and loads. With `group`
 * they are put in that group ('' for none); otherwise they keep theirs.
 */
export function moveWires(antenna: Antenna, selection: number[], at: number, group?: string): Moved {
  const sel = new Set(selection.filter((i) => i >= 0 && i < antenna.wires.length));
  const ids = antenna.wires.map((_, i) => i);
  const picked = ids.filter((i) => sel.has(i));
  const order = [...ids.filter((i) => i < at && !sel.has(i)), ...picked, ...ids.filter((i) => i >= at && !sel.has(i))];
  const moved: number[] = [];
  order.forEach((from, to) => (moved[from] = to));
  const wires = order.map((i) => (sel.has(i) && group !== undefined ? withGroup(antenna.wires[i], group) : antenna.wires[i]));
  const r = renumberWires(antenna, wires, (p) => followWire(p, moved));
  return { ...r, moved, selection: picked.map((i) => moved[i]) };
}

/** The wire in `group`; the field is left out for none, as the engine writes it. */
export function withGroup(w: Wire, group: string): Wire {
  const { group: _, ...rest } = w;
  return group ? { ...rest, group } : rest;
}

/** Radius and segmentation for a new wire: those of `like`, else MMANA's for the first wire. */
export function wireFrom(like: Wire | undefined, start: Point3, end: Point3): Wire {
  const base = like ? { radius: like.radius, segmentation: structuredClone(like.segmentation) } : firstWire(start, end);
  return { radius: base.radius, segmentation: base.segmentation, start: cleanPoint(start), end: cleanPoint(end), note: '' };
}

/** Where the wire list's 追加 puts a new wire. */
export type NewWireMode = 'zero' | 'continue' | 'offset' | 'fixed';

export interface NewWireSettings {
  mode: NewWireMode;
  /** For `offset`: the shift from the last wire. */
  offset: Point3;
  /** For `fixed`. */
  start: Point3;
  end: Point3;
}

/**
 * The wire the wire list adds after `last`. Like MMANA, it takes the radius
 * and segmentation of the last wire (here its group too, so it stays in the
 * group the list ends with), or MMANA's first-wire defaults; MMANA's new row
 * is the zero-length wire at the origin. Without a last wire, `continue` and `offset` start a 1 m wire at
 * the origin.
 */
export function addedWire(last: Wire | undefined, s: NewWireSettings): Wire {
  const origin = { x: 0, y: 0, z: 0 };
  const unit = (start: Point3) => ({ start, end: { ...start, y: start.y + 1 } });
  const ends =
    s.mode === 'zero'
      ? { start: origin, end: origin }
      : s.mode === 'fixed'
        ? { start: s.start, end: s.end }
        : !last
          ? unit(origin)
          : s.mode === 'continue'
            ? unit(last.end)
            : { start: add(last.start, s.offset), end: add(last.end, s.offset) };
  return { ...structuredClone(last ?? firstWire(origin, origin)), start: cleanPoint(ends.start), end: cleanPoint(ends.end), note: '' };
}

/** Wires along a polyline; zero-length steps are dropped. */
export function polylineWires(points: Point3[], like: Wire | undefined): Wire[] {
  const out: Wire[] = [];
  for (let i = 1; i < points.length; i++) {
    if (distance(points[i - 1], points[i]) < JOIN_EPS) continue;
    out.push(wireFrom(like, points[i - 1], points[i]));
  }
  return out;
}

/** Corners of the rectangle with opposite corners `a` and `b` on `plane`, closed (first = last). */
export function rectanglePoints(a: Point3, b: Point3, plane: Plane): Point3[] {
  const [u, v] = PLANE_AXES[plane];
  const c = (pu: number, pv: number): Point3 => ({ ...a, [u]: pu, [v]: pv });
  return [c(a[u], a[v]), c(b[u], a[v]), c(b[u], b[v]), c(a[u], b[v]), c(a[u], a[v])];
}

/**
 * A regular polygon on `plane` with its centre at `center` and one corner at
 * `corner`, closed (first = last). The corners are exact copies, so the loop
 * is closed for the engine.
 */
export function polygonPoints(center: Point3, corner: Point3, plane: Plane, sides: number): Point3[] {
  const turn = rotation(center, unitAxis(PLANE_NORMAL[plane]), 360 / sides);
  const pts = [cleanPoint(corner)];
  for (let i = 1; i < sides; i++) pts.push(cleanPoint(turn(pts[i - 1])));
  pts.push(pts[0]);
  return pts;
}

/** The wires joined to `start` through shared ends (an element or a loop). */
export function connectedWires(wires: Wire[], start: number): number[] {
  const seen = new Set([start]);
  const queue = [start];
  while (queue.length) {
    const w = wires[queue.pop()!];
    wires.forEach((o, i) => {
      if (seen.has(i)) return;
      if ([o.start, o.end].some((p) => samePoint(p, w.start) || samePoint(p, w.end))) {
        seen.add(i);
        queue.push(i);
      }
    });
  }
  return [...seen].sort((a, b) => a - b);
}

export const snapValue = (v: number, step: number) => (step > 0 ? Math.round(v / step) * step : v);

/** Snaps the in-plane coordinates of `p` to the grid; the plane's own coordinate is kept. */
export function snapToGrid(p: Point3, plane: Plane, step: number): Point3 {
  const [u, v] = PLANE_AXES[plane];
  return cleanPoint({ ...p, [u]: snapValue(p[u], step), [v]: snapValue(p[v], step) });
}

/**
 * Keeps the direction from `from` to `to` on multiples of `deg` within the
 * plane, at the same distance (Shift while drawing).
 */
export function snapAngle(from: Point3, to: Point3, plane: Plane, deg: number): Point3 {
  const [u, v] = PLANE_AXES[plane];
  const du = to[u] - from[u];
  const dv = to[v] - from[v];
  const r = Math.hypot(du, dv);
  if (r === 0) return to;
  const step = (deg * Math.PI) / 180;
  const a = Math.round(Math.atan2(dv, du) / step) * step;
  return cleanPoint({ ...to, [u]: from[u] + r * Math.cos(a), [v]: from[v] + r * Math.sin(a) });
}

/** Keeps only the largest component of a move (Shift while dragging). */
export function dominantAxis(d: Point3): Point3 {
  const k = AXES.reduce((m, a) => (Math.abs(d[a]) > Math.abs(d[m]) ? a : m), 'x' as Axis);
  return { x: 0, y: 0, z: 0, [k]: d[k] };
}

/** A round grid step near `x` (1, 2 or 5 × 10^n). */
export function niceStep(x: number): number {
  if (!(x > 0)) return 0.1;
  const e = Math.floor(Math.log10(x));
  const f = x / 10 ** e;
  const m = f < 1.5 ? 1 : f < 3.5 ? 2 : f < 7.5 ? 5 : 10;
  return Number((m * 10 ** e).toPrecision(1));
}

export interface Pt2 {
  x: number;
  y: number;
}

export function distToSegment2(p: Pt2, a: Pt2, b: Pt2): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const l2 = dx * dx + dy * dy;
  const t = l2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/** Whether a screen segment touches an axis-aligned rectangle (box selection). */
export function segmentHitsRect(a: Pt2, b: Pt2, r: { x0: number; y0: number; x1: number; y1: number }): boolean {
  const inside = (p: Pt2) => p.x >= r.x0 && p.x <= r.x1 && p.y >= r.y0 && p.y <= r.y1;
  if (inside(a) || inside(b)) return true;
  // Liang–Barsky clipping
  let t0 = 0;
  let t1 = 1;
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const clip = (p: number, q: number) => {
    if (p === 0) return q >= 0;
    const t = q / p;
    if (p < 0) {
      if (t > t1) return false;
      t0 = Math.max(t0, t);
    } else {
      if (t < t0) return false;
      t1 = Math.min(t1, t);
    }
    return true;
  };
  return clip(-dx, a.x - r.x0) && clip(dx, r.x1 - a.x) && clip(-dy, a.y - r.y0) && clip(dy, r.y1 - a.y) && t0 <= t1;
}
