import { describe, expect, it } from 'vitest';
import type { SweepPoint } from '../api/types';
import { newWire } from '../domain/project';
import { BODY_HEIGHT, ROW_HEIGHT, extentOf, paginate, swrAgainst, swrBandwidth } from '../domain/print';

describe('placing sections on sheets', () => {
  it('keeps blocks on one sheet while they fit, with gaps between them', () => {
    expect(paginate([{ key: 'a', height: 100 }, { key: 'b', height: 100 }], 210, 5)).toEqual([
      [
        { key: 'a', height: 100 },
        { key: 'b', height: 100 },
      ],
    ]);
  });

  it('starts a sheet for a block that does not fit', () => {
    const sheets = paginate([{ key: 'a', height: 150 }, { key: 'b', height: 100 }], 200, 5);
    expect(sheets.map((s) => s.map((p) => p.key))).toEqual([['a'], ['b']]);
  });

  it('cuts a table between rows and runs it on to the next sheet', () => {
    const sheets = paginate([{ key: 't', head: 10, rows: 100 }], 10 + 40 * ROW_HEIGHT, 5);
    expect(sheets.map((s) => s.map((p) => p.rows))).toEqual([[{ start: 0, end: 40 }], [{ start: 40, end: 80 }], [{ start: 80, end: 100 }]]);
  });

  it('moves a table to the next sheet rather than leave a row or two at the foot', () => {
    const body = 100;
    const sheets = paginate([{ key: 'a', height: body - 10 - 5 - 2 * ROW_HEIGHT - 1 }, { key: 't', head: 10, rows: 20 }], body, 5);
    expect(sheets[0].map((p) => p.key)).toEqual(['a']);
    expect(sheets[1][0].rows?.start).toBe(0);
  });

  it('lets a short table end at the foot of a sheet', () => {
    const body = 100;
    const sheets = paginate([{ key: 'a', height: body - 10 - 5 - 2 * ROW_HEIGHT - 1 }, { key: 't', head: 10, rows: 2 }], body, 5);
    expect(sheets).toHaveLength(1);
  });

  it('has room for about fifty rows on a sheet', () => {
    expect(Math.floor((BODY_HEIGHT - 10) / ROW_HEIGHT)).toBeGreaterThan(45);
  });
});

describe("MMANA's printed figures", () => {
  it('gives the SWR against 600 Ω', () => {
    expect(swrAgainst({ re: 600, im: 0 }, 600)).toBeCloseTo(1, 9);
    expect(swrAgainst({ re: 50, im: 0 }, 600)).toBeCloseTo(12, 9);
    expect(swrAgainst({ re: 0, im: 10 }, 600)).toBe(Infinity);
  });

  const sweep = (swrs: number[]) => swrs.map((swr, i) => ({ frequencyMhz: 14 + i * 0.1, swr }) as SweepPoint);

  it('interpolates the band edges around the lowest SWR', () => {
    const bw = swrBandwidth(sweep([3, 2, 1.2, 1.4, 2.5]), 1.5);
    expect(bw?.lowMhz).toBeCloseTo(14.1 + (0.1 * 0.5) / 0.8, 9);
    expect(bw?.highMhz).toBeCloseTo(14.3 + (0.1 * 0.1) / 1.1, 9);
    expect(bw?.open).toBe(false);
  });

  it('says when the band reaches an end of the sweep', () => {
    const bw = swrBandwidth(sweep([1.1, 1.3, 3]), 2);
    expect(bw?.lowMhz).toBe(14);
    expect(bw?.open).toBe(true);
  });

  it('gives no band when the SWR never gets under the limit', () => {
    expect(swrBandwidth(sweep([3, 2.5, 3]), 2)).toBeNull();
  });

  it('boxes every wire', () => {
    const e = extentOf([newWire({ x: -1, y: 0, z: 2 }, { x: 3, y: -4, z: 2 }), newWire({ x: 0, y: 5, z: 0 }, { x: 0, y: 5, z: 1 })]);
    expect(e).toEqual({ min: { x: -1, y: -4, z: 0 }, max: { x: 3, y: 5, z: 2 } });
    expect(extentOf([])).toBeNull();
  });
});
