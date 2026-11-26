// Order in which a sweep's points are calculated: a coarse outline first
// (both ends, the quarters and the centre, like MMANA's five-point 周波数特性),
// then the midpoint of the widest remaining gap, so a large model shows the
// shape of the curve long before every point is done.

/** Points of the outline calculated first. */
export const OUTLINE_POINTS = 5;

/** Indices 0..points-1, outline first, then filling the widest gap. */
export function sweepOrder(points: number): number[] {
  const n = Math.max(1, Math.floor(points));
  const outline = [...new Set(Array.from({ length: OUTLINE_POINTS }, (_, k) => Math.round((k * (n - 1)) / (OUTLINE_POINTS - 1))))];
  const order = [...outline];
  // computed indices, ascending
  const done = [...outline].sort((a, b) => a - b);
  while (order.length < n) {
    let at = 0;
    for (let i = 1; i < done.length - 1; i++) {
      if (done[i + 1] - done[i] > done[at + 1] - done[at]) at = i;
    }
    const mid = Math.floor((done[at] + done[at + 1]) / 2);
    order.push(mid);
    done.splice(at + 1, 0, mid);
  }
  return order;
}

// ------------------------------------------------------------ matching ---
// The 周波数特性 dialog's マッチング: an L network designed at one frequency
// and applied across the sweep (sweepMatch in mmana-core's circuits.rs).

/** Which charts show the sweep through the network (なし・SWR のみ・Z と SWR). */
export type MatchMode = 'off' | 'swr' | 'swrZ';

export interface MatchSettings {
  mode: MatchMode;
  /** Match at the middle of the sweep (中心周波数でマッチングを取る), else at `frequencyMhz`. */
  atCentre: boolean;
  frequencyMhz: number;
}

export interface SweepRange {
  startMhz: number;
  stopMhz: number;
  points: number;
}

/** Frequency of point `i`, spaced as the engine spaces them (`sweep_frequency`). */
export const sweepFrequency = (r: SweepRange, i: number) =>
  r.points <= 1 ? r.startMhz : r.startMhz + ((r.stopMhz - r.startMhz) * i) / (r.points - 1);

/** Where the network is designed: the middle point of the sweep (the midpoint of an even one), or the frequency entered. */
export function matchFrequency(r: SweepRange, m: MatchSettings): number {
  if (!m.atCentre) return m.frequencyMhz;
  return r.points % 2 === 1 ? sweepFrequency(r, (r.points - 1) / 2) : (r.startMhz + r.stopMhz) / 2;
}

/** The point at `mhz` among `points`, if one was calculated there. */
export function pointAt<P extends { frequencyMhz: number }>(points: P[], mhz: number): P | undefined {
  return points.find((p) => Math.abs(p.frequencyMhz - mhz) <= 1e-9 * Math.abs(mhz));
}

/** The middle point is one of the sweep's own, calculated with the outline. */
export const centreIsAPoint = (r: SweepRange) => r.points % 2 === 1;

