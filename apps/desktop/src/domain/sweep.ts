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
