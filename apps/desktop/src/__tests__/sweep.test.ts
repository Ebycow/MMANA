import { describe, expect, it } from 'vitest';
import { matchFrequency, pointAt, sweepFrequency } from '../domain/sweep';

const range = { startMhz: 13.8, stopMhz: 14.4, points: 21 };

describe('the matching frequency', () => {
  it('is the middle point of an odd sweep, spaced as the engine spaces it', () => {
    const f = matchFrequency(range, { mode: 'swr', atCentre: true, frequencyMhz: 7 });
    expect(f).toBe(13.8 + ((14.4 - 13.8) * 10) / 20);
    expect(pointAt([{ frequencyMhz: sweepFrequency(range, 10) }], f)).toBeDefined();
  });

  it('is the midpoint of an even sweep, which no point lies on', () => {
    const even = { ...range, points: 20 };
    const f = matchFrequency(even, { mode: 'swr', atCentre: true, frequencyMhz: 7 });
    expect(f).toBeCloseTo(14.1, 12);
    const points = Array.from({ length: 20 }, (_, i) => ({ frequencyMhz: sweepFrequency(even, i) }));
    expect(pointAt(points, f)).toBeUndefined();
  });

  it('is the frequency entered when not at the centre', () => {
    expect(matchFrequency(range, { mode: 'swrZ', atCentre: false, frequencyMhz: 14.2 })).toBe(14.2);
  });
});
