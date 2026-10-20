// Polar geometry shared by the pattern plots (pure, unit-tested).
//
// Radius follows MMANA (CDrawPtnH/V::GetDBR): r = R · 10^(dB/40), i.e. the
// square root of the field ratio, with dB relative to the overall maximum.
//   horizontal: azimuth 0° at the top, increasing clockwise (+X forward)
//   vertical:   front horizon right, zenith top, back horizon left

export type CutKind = 'horizontal' | 'vertical';

export const radiusFor = (db: number, maxDb: number) => Math.pow(10, Math.min(0, db - maxDb) / 40);

/** Plot angle (radians, screen: 0 = right, counter-clockwise) of sample i. */
export function sampleAngle(kind: CutKind, i: number, step: number): number {
  const deg = i * step;
  return kind === 'horizontal' ? ((90 - deg) * Math.PI) / 180 : ((180 - deg) * Math.PI) / 180;
}

/** Nearest sample index for a pointer at (dx, dy) from the centre (screen y down). */
export function sampleAt(kind: CutKind, dx: number, dy: number, step: number, count: number): number | null {
  const a = (Math.atan2(-dy, dx) * 180) / Math.PI; // CCW from right
  let deg: number;
  if (kind === 'horizontal') {
    deg = (90 - a + 360) % 360;
  } else {
    if (-dy < 0) return null; // below the horizon
    deg = 180 - a;
  }
  const i = Math.round(deg / step);
  return i >= 0 && i < count ? i : kind === 'horizontal' ? 0 : null;
}

/** Human-readable label of sample i. */
export function sampleLabel(kind: CutKind, i: number, step: number): string {
  const deg = i * step;
  if (kind === 'horizontal') return `方位 ${deg}°`;
  return deg <= 90 ? `仰角 ${deg}°（後方）` : `仰角 ${180 - deg}°（前方）`;
}

export const RINGS_DB = [0, -3, -6, -10, -20, -30];
