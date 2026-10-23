// Where wires really join. The engine joins two wires only where their ends
// are exactly equal (`S1299`), so an end lying on the middle of another wire,
// two ends a hair apart or two wires that cross look joined in the view but
// are open circuits in the calculation. These helpers find such places and
// split wires so that they join, renaming the pulses of sources and loads on
// a split wire to the same place on its pieces.
import type { Antenna, Point3, Wire } from '../api/types';
import { normalizePulse } from './geometry';
import { add, cross, distance, dot, followWire, JOIN_EPS, renumberWires, samePoint, scale, sub, type End, type FollowResult } from './wireEdit';

export interface WireEnd {
  wire: number;
  end: End;
}

export type JunctionIssue =
  /** The end `by` touches the middle of `wire`, which does not join it. */
  | { kind: 'tee'; wire: number; by: WireEnd; at: Point3 }
  /** Ends that touch but are not exactly the same point; `at` is where they meet. */
  | { kind: 'gap'; ends: WireEnd[]; at: Point3; distance: number }
  /** Two wires that touch between their ends. */
  | { kind: 'cross'; wires: [number, number]; at: Point3 }
  /** Two wires with the same ends; `wires[1]` is the one to drop. */
  | { kind: 'overlap'; wires: [number, number] };

/** Ends closer than this always touch, whatever the radii (0.1 mm). */
const MIN_CONTACT = 1e-4;

/**
 * Whether a wire has a body of its own: zero disables it and a negative
 * radius names a combined wire, whose sections run from its start and so
 * cannot be split.
 */
const solid = (w: Wire) => w.radius > 0;

/** How close two wires must come to touch: the sum of their radii. */
const contact = (a: Wire, b: Wire) => Math.max(Math.max(a.radius, 0) + Math.max(b.radius, 0), MIN_CONTACT);

const exactly = (a: Point3, b: Point3) => a.x === b.x && a.y === b.y && a.z === b.z;

const sameWire = (a: Wire, b: Wire) =>
  (exactly(a.start, b.start) && exactly(a.end, b.end)) || (exactly(a.start, b.end) && exactly(a.end, b.start));

/** Where `p` falls along `w` (0 at its start, 1 at its end), the nearest point there and how far `p` is from it. */
export function along(w: Wire, p: Point3): { t: number; at: Point3; dist: number } {
  const d = sub(w.end, w.start);
  const l2 = dot(d, d);
  const t = l2 === 0 ? 0 : Math.max(0, Math.min(1, dot(sub(p, w.start), d) / l2));
  const at = add(w.start, scale(d, t));
  return { t, at, dist: distance(p, at) };
}

/** Whether `p` lies on `w` away from both its ends, within `tol`. */
function onMiddle(w: Wire, p: Point3, tol: number): Point3 | null {
  const a = along(w, p);
  if (a.dist > tol || distance(a.at, w.start) <= tol || distance(a.at, w.end) <= tol) return null;
  return a.at;
}

/** The closest points of two segments, or null when they are (nearly) parallel. */
function closest(a: Wire, b: Wire): { p: Point3; q: Point3 } | null {
  const d1 = sub(a.end, a.start);
  const d2 = sub(b.end, b.start);
  const r = sub(a.start, b.start);
  const aa = dot(d1, d1);
  const ee = dot(d2, d2);
  const n = cross(d1, d2);
  if (aa === 0 || ee === 0 || dot(n, n) <= 1e-12 * aa * ee) return null;
  const bb = dot(d1, d2);
  const c = dot(d1, r);
  const f = dot(d2, r);
  const den = aa * ee - bb * bb;
  const clamp = (v: number) => Math.max(0, Math.min(1, v));
  let s = clamp((bb * f - c * ee) / den);
  let t = (bb * s + f) / ee;
  if (t < 0 || t > 1) {
    t = clamp(t);
    s = clamp((t * bb - c) / aa);
  }
  return { p: add(a.start, scale(d1, s)), q: add(b.start, scale(d2, t)) };
}

const ENDS: End[] = ['start', 'end'];

/**
 * The places where wires touch but the engine does not join them. Wires
 * touch where they come within the sum of their radii (at least 0.1 mm).
 */
export function findJunctionIssues(wires: Wire[]): JunctionIssue[] {
  const issues: JunctionIssue[] = [];
  const live = wires.map((_, i) => i).filter((i) => wires[i].radius !== 0);

  // ends that touch: groups of ends linked by contact, unless all are the same point
  const ends = live.flatMap((i) => ENDS.map((end): WireEnd => ({ wire: i, end })));
  const point = (e: WireEnd) => wires[e.wire][e.end];
  const parent = ends.map((_, k) => k);
  const root = (k: number): number => (parent[k] === k ? k : (parent[k] = root(parent[k])));
  for (let a = 0; a < ends.length; a++) {
    for (let b = a + 1; b < ends.length; b++) {
      if (ends[a].wire === ends[b].wire) continue;
      if (distance(point(ends[a]), point(ends[b])) <= contact(wires[ends[a].wire], wires[ends[b].wire])) parent[root(a)] = root(b);
    }
  }
  const groups = new Map<number, WireEnd[]>();
  ends.forEach((e, k) => groups.set(root(k), [...(groups.get(root(k)) ?? []), e]));
  const gapped = new Set<string>();
  for (const g of groups.values()) {
    if (g.length < 2 || g.every((e) => exactly(point(e), point(g[0])))) continue;
    // a wire shorter than the contact distance cannot be closed up
    if (new Set(g.map((e) => e.wire)).size < g.length) continue;
    const at = gapTarget(g.map(point));
    issues.push({ kind: 'gap', ends: g, at, distance: Math.max(...g.map((e) => distance(point(e), at))) });
    for (const e of g) gapped.add(`${e.wire}${e.end}`);
  }

  for (const i of live) {
    for (const j of live) {
      if (i === j) continue;
      for (const end of ENDS) {
        if (gapped.has(`${i}${end}`)) continue;
        const at = onMiddle(wires[j], wires[i][end], contact(wires[i], wires[j]));
        if (at) issues.push({ kind: 'tee', wire: j, by: { wire: i, end }, at });
      }
    }
  }

  for (let a = 0; a < live.length; a++) {
    for (let b = a + 1; b < live.length; b++) {
      const [i, j] = [live[a], live[b]];
      if (sameWire(wires[i], wires[j])) {
        issues.push({ kind: 'overlap', wires: [i, j] });
        continue;
      }
      const c = closest(wires[i], wires[j]);
      if (!c) continue;
      const tol = contact(wires[i], wires[j]);
      if (distance(c.p, c.q) > tol) continue;
      const at = scale(add(c.p, c.q), 0.5);
      // touching at an end is a tee or a gap, found above
      if ([wires[i], wires[j]].some((w) => ENDS.some((e) => distance(w[e], at) <= tol))) continue;
      issues.push({ kind: 'cross', wires: [i, j], at });
    }
  }
  return issues;
}

/** The point most of the ends already share (the first of them on a tie). */
function gapTarget(points: Point3[]): Point3 {
  let best = points[0];
  let count = 0;
  for (const p of points) {
    const n = points.filter((q) => exactly(p, q)).length;
    if (n > count) {
      best = p;
      count = n;
    }
  }
  return best;
}

/**
 * Whether an issue is most likely a mistake. Crossing wires are not: MMANA
 * models a transposed phasing line (HB9CV, LPDA) as two wires crossing in
 * one plane, which the engine leaves unjoined as intended.
 */
export const isMistake = (issue: JunctionIssue) => issue.kind !== 'cross';

/** Whether an issue can be fixed here (combined wires are not split). */
export function fixable(wires: Wire[], issue: JunctionIssue): boolean {
  switch (issue.kind) {
    case 'tee':
      return solid(wires[issue.wire]);
    case 'cross':
      return issue.wires.every((i) => solid(wires[i]));
    default:
      return true;
  }
}

export interface Cut {
  wire: number;
  /** The point to cut at, used exactly as the ends of the new pieces. */
  at: Point3;
}

export interface Split {
  antenna: Antenna;
  /** The new indices of each old wire, from its start (one when it was not split, none when it was dropped). */
  pieces: number[][];
  warnings: string[];
}

/**
 * Cuts wires into pieces at the given points, keeping the order of the
 * wires (pieces of wire i take its place). Sources and loads follow: those
 * on a split wire go to the same pulse on its pieces where there is one
 * (the centre of a wire cut in half is `wNe` of the first half, as the
 * engine names that pulse), else to the nearest centre with a warning.
 */
export function splitWires(antenna: Antenna, cuts: Cut[]): Split {
  const at = new Map<number, { t: number; p: Point3 }[]>();
  for (const c of cuts) {
    const w = antenna.wires[c.wire];
    if (!w || !solid(w)) continue;
    const a = along(w, c.at);
    if (distance(c.at, w.start) < JOIN_EPS || distance(c.at, w.end) < JOIN_EPS) continue;
    const list = at.get(c.wire) ?? [];
    if (!list.some((q) => samePoint(q.p, c.at))) list.push({ t: a.t, p: c.at });
    at.set(c.wire, list);
  }
  const wires: Wire[] = [];
  const pieces: number[][] = [];
  const bounds: number[][] = [];
  antenna.wires.forEach((w, i) => {
    const list = (at.get(i) ?? []).sort((a, b) => a.t - b.t);
    const ts = [0, ...list.map((q) => q.t), 1];
    const pts = [w.start, ...list.map((q) => q.p), w.end];
    pieces.push([]);
    bounds.push(ts);
    for (let k = 1; k < pts.length; k++) {
      pieces[i].push(wires.length);
      wires.push({ ...structuredClone(w), start: pts[k - 1], end: pts[k], segmentation: share(w, ts[k] - ts[k - 1]) });
    }
  });
  if (at.size === 0) return { antenna, pieces, warnings: [] };
  const moved = pieces.map((p) => p[0]);
  const rename = (pulse: string): FollowResult => {
    const m = /^([wW])(\d+)(.*)$/.exec(normalizePulse(pulse));
    const n = m ? Number(m[2]) : 0;
    if (!m || n === 0 || n > pieces.length || pieces[n - 1].length === 1) return followWire(pulse, moved);
    return splitPulse(pulse, m[1], m[3], pieces[n - 1], bounds[n - 1]);
  };
  const r = renumberWires(antenna, wires, rename);
  return { antenna: r.antenna, pieces, warnings: r.warnings };
}

/** The segmentation of a piece covering `share` of `w`: fixed counts are shared out, automatic ones kept. */
function share(w: Wire, part: number): Wire['segmentation'] {
  const s = w.segmentation;
  return s.kind === 'fixed' ? { kind: 'fixed', count: Math.max(1, Math.round(s.count * part)) } : structuredClone(s);
}

/**
 * The name on the pieces of a split wire (`pieces`: new indices, `ts`: where
 * they start and end along the old wire) of the pulse `wN<rest>`.
 */
function splitPulse(pulse: string, w: string, rest: string, pieces: number[], ts: number[]): FollowResult {
  const m = /^([bBcCeE]?)(.*)$/.exec(rest)!;
  const kind = (m[1] || 'c').toLowerCase();
  const name = (k: number, suffix: string) => `${w}${pieces[k] + 1}${suffix}`;
  // counted from an end of the wire: the piece at that end, counted the same way
  if (kind === 'b') return { pulse: name(0, rest), keep: true };
  if (kind === 'e') return { pulse: name(pieces.length - 1, rest), keep: true };
  const centre = m[2].trim() === '' || Number(m[2]) === 0;
  const cut = ts.findIndex((t, k) => k > 0 && k < ts.length - 1 && Math.abs(t - 0.5) < 1e-9);
  if (centre && cut > 0) {
    const to = name(cut - 1, 'e');
    return { pulse: to, keep: true, warning: `「${pulse}」は分けたワイヤーのつなぎ目にあるため、同じ位置の「${to}」にしました` };
  }
  const k = Math.max(0, ts.findIndex((t) => t > 0.5) - 1);
  const to = name(k, 'c');
  if (centre && Math.abs((ts[k] + ts[k + 1]) / 2 - 0.5) < 1e-9) return { pulse: to, keep: true };
  return { pulse: to, keep: true, warning: `「${pulse}」のあったワイヤーを分けたため、「${to}」（分けたワイヤーの中央）に移しました。位置を確かめてください` };
}

/**
 * Joins the wires at `touched` (new or moved) to the others: an end of
 * either that lies on the middle of the other cuts it there, so drawing a
 * wire from the middle of another, or dropping an end on it, makes a real
 * junction. A wire lying exactly on another (a copy made in place) is left
 * alone.
 */
export function joinTouched(antenna: Antenna, touched: number[]): Split {
  const wires = antenna.wires;
  const cuts: Cut[] = [];
  const on = (j: number, p: Point3) => {
    if (solid(wires[j]) && onMiddle(wires[j], p, JOIN_EPS)) cuts.push({ wire: j, at: p });
  };
  for (const i of new Set(touched)) {
    const w = wires[i];
    if (!w || w.radius === 0 || wires.some((o, j) => j !== i && sameWire(o, w))) continue;
    wires.forEach((o, j) => {
      if (j === i || o.radius === 0) return;
      for (const end of ENDS) {
        on(j, w[end]);
        on(i, o[end]);
      }
    });
  }
  return splitWires(antenna, cuts);
}

/**
 * Fixes the issues as one edit: ends that touch are moved onto one point,
 * an end touching the middle of a wire is moved onto it and the wire cut
 * there, crossing wires are cut at their crossing and the second of two
 * overlapping wires is dropped (its sources and loads go to the first).
 * Issues that are not `fixable` are skipped.
 */
export function fixJunctions(antenna: Antenna, issues: JunctionIssue[]): Split {
  const wires = structuredClone(antenna.wires);
  const cuts: Cut[] = [];
  const drop = new Map<number, { keep: number; reversed: boolean }>();
  for (const issue of issues) {
    if (!fixable(antenna.wires, issue)) continue;
    if (issue.kind === 'gap') {
      for (const e of issue.ends) wires[e.wire][e.end] = { ...issue.at };
    } else if (issue.kind === 'tee') {
      wires[issue.by.wire][issue.by.end] = { ...issue.at };
      cuts.push({ wire: issue.wire, at: issue.at });
    } else if (issue.kind === 'cross') {
      for (const i of issue.wires) cuts.push({ wire: i, at: issue.at });
    } else {
      const [keep, gone] = issue.wires;
      if (!drop.has(keep) && !drop.has(gone)) drop.set(gone, { keep, reversed: !exactly(antenna.wires[keep].start, antenna.wires[gone].start) });
    }
  }
  // drop the overlapping copies first, sending their pulses to the wire they lay on
  const moved: (number | null)[] = [];
  let next = 0;
  for (let i = 0; i < wires.length; i++) moved.push(drop.has(i) ? null : next++);
  const rename = (pulse: string): FollowResult => {
    const m = /^([wW])(\d+)([bBeE]?)(.*)$/.exec(normalizePulse(pulse));
    const d = m ? drop.get(Number(m[2]) - 1) : undefined;
    if (!m || !d) return followWire(pulse, moved);
    const kind = d.reversed ? { b: 'e', B: 'E', e: 'b', E: 'B' }[m[3]] ?? m[3] : m[3];
    return { pulse: `${m[1]}${moved[d.keep]! + 1}${kind}${m[4]}`, keep: true };
  };
  const kept = renumberWires({ ...antenna, wires }, wires.filter((_, i) => !drop.has(i)), rename);
  const split = splitWires(
    kept.antenna,
    cuts.flatMap((c) => (moved[c.wire] == null ? [] : [{ wire: moved[c.wire]!, at: c.at }])),
  );
  const pieces = moved.map((i) => (i == null ? [] : split.pieces[i]));
  return { antenna: split.antenna, pieces, warnings: [...kept.warnings, ...split.warnings] };
}

/** The wires an issue is about, for selecting them. */
export function issueWires(issue: JunctionIssue): number[] {
  switch (issue.kind) {
    case 'tee':
      return [issue.wire, issue.by.wire];
    case 'gap':
      return [...new Set(issue.ends.map((e) => e.wire))];
    default:
      return [...issue.wires];
  }
}

/** Where to mark an issue in the view. */
export function issuePoint(wires: Wire[], issue: JunctionIssue): Point3 {
  if (issue.kind === 'overlap') {
    const w = wires[issue.wires[1]];
    return scale(add(w.start, w.end), 0.5);
  }
  return issue.at;
}

const endName = (e: WireEnd) => `ワイヤー ${e.wire + 1} の${e.end === 'start' ? '始点' : '終点'}`;
const mm = (m: number) => (m < 0.001 ? `${Number((m * 1000).toPrecision(2))} mm` : `${Number((m * 1000).toFixed(1))} mm`);

/** One line for the list of issues. */
export function describeIssue(issue: JunctionIssue): string {
  switch (issue.kind) {
    case 'tee':
      return `${endName(issue.by)}がワイヤー ${issue.wire + 1} の途中に触れていますが、つながっていません`;
    case 'gap':
      return `${issue.ends.map(endName).join('、')}が ${mm(issue.distance)} ずれていて、つながっていません`;
    case 'cross':
      return `ワイヤー ${issue.wires[0] + 1} と ${issue.wires[1] + 1} が途中で交わっています（つながっていません。たすき掛けの位相線ならこのままで正しい）`;
    case 'overlap':
      return `ワイヤー ${issue.wires[0] + 1} と ${issue.wires[1] + 1} が重なっています`;
  }
}
