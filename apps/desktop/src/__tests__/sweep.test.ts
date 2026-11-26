import { describe, expect, it } from 'vitest';
import { PATTERN_SLOTS, initialPatterns, matchFrequency, networkText, outlineIndices, pointAt, sweepFrequency, togglePattern } from '../domain/sweep';

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

describe('the network as the panel names it', () => {
  it('goes from the antenna to the line', () => {
    const c = { kind: 'C' as const, value: 457.53962, reactanceOhm: -24.76 };
    const l = { kind: 'L' as const, value: 0.5373162, reactanceOhm: 47.43 };
    expect(networkText({ shuntSide: 'line', series: c, shunt: l })).toBe('アンテナに直列 C 457.5 pF、給電線側に並列 L 0.5373 µH');
    expect(networkText({ shuntSide: 'antenna', shunt: c, series: l })).toBe('アンテナに並列 C 457.5 pF、給電線へ直列 L 0.5373 µH');
    expect(networkText({ shuntSide: 'line', series: c })).toBe('アンテナに直列 C 457.5 pF');
    expect(networkText({ shuntSide: 'line' })).toBe('整合済み（素子なし）');
  });
});

describe('the patterns shown across the band', () => {
  it('start from the outline: both ends, the quarters and the middle', () => {
    expect(outlineIndices(21)).toEqual([0, 5, 10, 15, 20]);
    expect(outlineIndices(3)).toEqual([0, 1, 2]);
    const f = Array.from({ length: 21 }, (_, i) => sweepFrequency(range, i));
    expect([...initialPatterns(f)]).toEqual([0, 5, 10, 15, 20].map((i, slot) => [f[i], slot]));
  });

  it('keep their colours when another is hidden or shown', () => {
    let s = initialPatterns([1, 2, 3, 4, 5]);
    expect(togglePattern(s, 6).has(6)).toBe(false); // every slot is taken
    s = togglePattern(s, 2);
    expect(s.has(2)).toBe(false);
    s = togglePattern(s, 9);
    expect(s.get(9)).toBe(1); // the slot 2 had
    expect([1, 3, 4, 5].map((f) => s.get(f))).toEqual([0, 2, 3, 4]);
    expect(s.size).toBe(PATTERN_SLOTS);
  });
});
