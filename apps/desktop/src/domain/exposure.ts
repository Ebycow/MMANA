// The radio-protection check of the 3D view (電波防護): the near electric and
// magnetic field on a horizontal plane at the height people are, against the
// limits of 電波法施行規則 第21条の4, 別表第二号の三の三.
import type { CalcResult, NearFieldResponse, Point3 } from '../api/types';
import type { Bounds } from './geometry';

type Sample = NearFieldResponse['samples'][number];

/** Points along each side of the plane at most. */
const MAX_SIDE = 101;
/** The automatic range never goes past this half width (m). */
const MAX_AUTO_HALF = 300;

/** Field strengths (RMS) not to be exceeded; a criterion is null outside its frequencies. */
export interface Limits {
  /** 第1: averaged over 6 minutes, so judged at the average power. */
  average: { e: number; h: number } | null;
  /** 第2: instantaneous, for stimulation (10 kHz–10 MHz), so judged at the peak envelope power. */
  instantaneous: { e: number; h: number } | null;
}

/** The peak power the check uses: an envelope never peaks below its average. */
export const peakPower = (s: ExposureSettings) => Math.max(s.peakW, s.averageW);

/** The limits at `f` MHz. */
export function limits(f: number): Limits {
  let average: Limits['average'] = null;
  if (f > 0.1 && f <= 3) average = { e: 275, h: 2.18 / f };
  else if (f > 3 && f <= 30) average = { e: 824 / f, h: 2.18 / f };
  else if (f > 30 && f <= 300) average = { e: 27.5, h: 0.0728 };
  else if (f > 300 && f <= 1500) average = { e: 1.585 * Math.sqrt(f), h: Math.sqrt(f) / 237.8 };
  else if (f > 1500 && f <= 300_000) average = { e: 61.4, h: 0.163 };
  const instantaneous = f > 0.01 && f <= 10 ? { e: 83, h: 21 } : null;
  return { average, instantaneous };
}

export interface ExposureSettings {
  /** Average power into the antenna (W), for the 6-minute limits. */
  averageW: number;
  /** Peak envelope power into the antenna (W), for the instantaneous limits; never taken below `averageW`. */
  peakW: number;
  /** Height of the plane above the ground (m); the Z of the plane in free space. */
  heightM: number;
  /** Half width of the plane (m); null chooses it from the power and the gain. */
  rangeM: number | null;
}

/** A square of `n` × `n` points `step` apart from (x0, y0), at height `z` in view coordinates. */
export interface ExposureGrid {
  x0: number;
  y0: number;
  step: number;
  n: number;
  z: number;
  center: { x: number; y: number };
}

/**
 * Half width that takes in where the limit can be exceeded, from the far
 * field: E = √(30·P·G) / R, doubled for a reflection off the ground, at the
 * power and limit that give the farthest distance.
 */
export function autoRange(box: Bounds, planeZ: number, gainDbi: number, s: ExposureSettings, lim: Limits): number {
  const g = Math.max(1, 10 ** (gainDbi / 10));
  const reach = (p: number, e: number) => (2 * Math.sqrt(30 * p * g)) / e;
  const r = Math.max(
    lim.average ? reach(s.averageW, lim.average.e) : 0,
    lim.instantaneous ? reach(peakPower(s), lim.instantaneous.e) : 0,
  );
  const dz = Math.max(0, Math.abs(box.center.z - planeZ) - (box.maxZ - box.minZ) / 2);
  const across = Math.sqrt(Math.max(0, r * r - dz * dz));
  return Math.min(MAX_AUTO_HALF, Math.max(across * 1.3, box.radius * 1.5, 2));
}

/** The plane round the antenna's footprint at height `z` (view coordinates). */
export function exposureGrid(box: Bounds, z: number, half: number): ExposureGrid {
  const n = MAX_SIDE;
  const step = (2 * half) / (n - 1);
  return { x0: box.center.x - half, y0: box.center.y - half, step, n, z, center: { x: box.center.x, y: box.center.y } };
}

/** The near-field request for `grid` at 1 W; the engine's Z includes the antenna height over a ground. */
export function exposureSpec(grid: ExposureGrid, kind: 'electric' | 'magnetic', heightOffset: number) {
  return {
    kind,
    powerW: 1,
    x: { start: grid.x0, step: grid.step, count: grid.n },
    y: { start: grid.y0, step: grid.step, count: grid.n },
    z: { start: grid.z + heightOffset, step: 0, count: 1 },
  };
}

export interface ExposureMap {
  /** Ratio to the strictest limit at each point (x fastest). */
  ratio: Float32Array;
  /** 1 where the point is too close to a wire for the field to be trusted. */
  unsure: Float32Array;
  /** Largest field on the plane among the points that can be trusted (RMS, at the average power). */
  maxE: number;
  maxH: number;
  /** Largest ratio among the points that can be trusted, and where it is. */
  maxRatio: number;
  maxAt: Point3 | null;
  /** Farthest horizontal distance from the middle of the antenna at which the limit is exceeded; null when nowhere. */
  exceedReach: number | null;
  /** The limit is exceeded at the edge of the plane, so the plane may not take it all in. */
  exceedsEdge: boolean;
}

/** |F| (RMS) at each point from its three components; the samples come x fastest, X/Y/Z per point. */
function sizes(samples: Sample[], points: number): Float64Array | null {
  if (samples.length !== points * 3) return null;
  const out = new Float64Array(points);
  for (let i = 0; i < points; i++) {
    let sum = 0;
    for (let c = 0; c < 3; c++) {
      const s = samples[i * 3 + c];
      sum += s.real * s.real + s.imag * s.imag;
    }
    out[i] = Math.sqrt(sum);
  }
  return out;
}

function distanceToSegment(p: Point3, a: Point3, b: Point3): number {
  const d = { x: b.x - a.x, y: b.y - a.y, z: b.z - a.z };
  const len2 = d.x * d.x + d.y * d.y + d.z * d.z;
  const t = len2 > 0 ? Math.min(1, Math.max(0, ((p.x - a.x) * d.x + (p.y - a.y) * d.y + (p.z - a.z) * d.z) / len2)) : 0;
  return Math.hypot(p.x - a.x - t * d.x, p.y - a.y - t * d.y, p.z - a.z - t * d.z);
}

/**
 * Within a segment's length of a wire, the field of the pulses the engine
 * divides it into is not the field of the wire; such points are marked and
 * left out of the maximum.
 */
export function unsurePoints(grid: ExposureGrid, wires: CalcResult['segmentedWires']): Float32Array {
  const out = new Float32Array(grid.n * grid.n);
  const near = wires
    .filter((w) => w.segments > 0)
    .map((w) => ({ w, reach: Math.hypot(w.end.x - w.start.x, w.end.y - w.start.y, w.end.z - w.start.z) / w.segments }));
  for (let i = 0; i < out.length; i++) {
    const p = { x: grid.x0 + (i % grid.n) * grid.step, y: grid.y0 + Math.floor(i / grid.n) * grid.step, z: grid.z };
    if (near.some(({ w, reach }) => distanceToSegment(p, w.start, w.end) < reach)) out[i] = 1;
  }
  return out;
}

/** The fields at 1 W scaled to the powers and set against the limits. */
export function exposureMap(
  electric: Sample[],
  magnetic: Sample[],
  grid: ExposureGrid,
  unsure: Float32Array,
  s: ExposureSettings,
  lim: Limits,
): ExposureMap | null {
  const points = grid.n * grid.n;
  const e = sizes(electric, points);
  const h = sizes(magnetic, points);
  if (!e || !h || unsure.length !== points) return null;
  const avg = Math.sqrt(s.averageW);
  const peak = Math.sqrt(peakPower(s));
  const ratio = new Float32Array(points);
  const out: ExposureMap = { ratio, unsure, maxE: 0, maxH: 0, maxRatio: 0, maxAt: null, exceedReach: null, exceedsEdge: false };
  for (let i = 0; i < points; i++) {
    let r = 0;
    if (lim.average) r = Math.max(r, (e[i] * avg) / lim.average.e, (h[i] * avg) / lim.average.h);
    if (lim.instantaneous) r = Math.max(r, (e[i] * peak) / lim.instantaneous.e, (h[i] * peak) / lim.instantaneous.h);
    ratio[i] = r;
    const ix = i % grid.n;
    const iy = Math.floor(i / grid.n);
    const x = grid.x0 + ix * grid.step;
    const y = grid.y0 + iy * grid.step;
    // a point too close to a wire still counts towards the reach: the field there is strong, if not known
    if (r >= 1) {
      out.exceedReach = Math.max(out.exceedReach ?? 0, Math.hypot(x - grid.center.x, y - grid.center.y));
      if (ix === 0 || iy === 0 || ix === grid.n - 1 || iy === grid.n - 1) out.exceedsEdge = true;
    }
    if (unsure[i]) continue;
    out.maxE = Math.max(out.maxE, e[i] * avg);
    out.maxH = Math.max(out.maxH, h[i] * avg);
    if (r > out.maxRatio) {
      out.maxRatio = r;
      out.maxAt = { x, y, z: grid.z };
    }
  }
  return out;
}
