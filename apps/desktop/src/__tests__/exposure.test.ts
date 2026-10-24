import { describe, expect, it } from 'vitest';
import type { NearFieldResponse } from '../api/types';
import { autoRange, exposureGrid, exposureMap, exposureSpec, limits, unsurePoints, type ExposureSettings } from '../domain/exposure';
import type { Bounds } from '../domain/geometry';

type Sample = NearFieldResponse['samples'][number];

describe('limits (電波法施行規則 別表第二号の三の三)', () => {
  it('follows the 6-minute table at the amateur bands', () => {
    expect(limits(1.9).average).toEqual({ e: 275, h: 2.18 / 1.9 });
    expect(limits(7.1).average!.e).toBeCloseTo(824 / 7.1);
    expect(limits(28.5).average!.e).toBeCloseTo(824 / 28.5);
    expect(limits(50).average).toEqual({ e: 27.5, h: 0.0728 });
    expect(limits(145).average).toEqual({ e: 27.5, h: 0.0728 });
    expect(limits(435).average!.e).toBeCloseTo(1.585 * Math.sqrt(435));
    expect(limits(435).average!.h).toBeCloseTo(Math.sqrt(435) / 237.8);
    expect(limits(2400).average).toEqual({ e: 61.4, h: 0.163 });
  });

  it('takes the upper end of each band into it', () => {
    expect(limits(3).average!.e).toBe(275);
    expect(limits(30).average!.e).toBeCloseTo(824 / 30);
    expect(limits(300).average!.e).toBe(27.5);
    expect(limits(1500).average!.e).toBeCloseTo(1.585 * Math.sqrt(1500));
  });

  it('adds the instantaneous table up to 10 MHz only', () => {
    expect(limits(7.1).instantaneous).toEqual({ e: 83, h: 21 });
    expect(limits(10).instantaneous).not.toBeNull();
    expect(limits(10.1).instantaneous).toBeNull();
    expect(limits(0.05)).toEqual({ average: null, instantaneous: { e: 83, h: 21 } });
  });
});

const box: Bounds = { center: { x: 0, y: 0, z: 20 }, radius: 5, minZ: 20, maxZ: 20 };
const settings: ExposureSettings = { averageW: 100, peakW: 100, heightM: 1.5, rangeM: null };

describe('plane', () => {
  it('reaches as far as the far field could exceed the limit', () => {
    // 145 MHz, 10 dBi, 100 W: 2·√(30·100·10) / 27.5 ≈ 12.6 m, 18.5 m below the antenna: not reached
    expect(autoRange(box, 1.5, 10, settings, limits(145))).toBe(7.5);
    // 1 kW at 20 dBi reaches 2·√(30·1000·100) / 27.5 ≈ 126 m
    const r = autoRange(box, 1.5, 20, { ...settings, averageW: 1000 }, limits(145));
    expect(r).toBeCloseTo(Math.sqrt(((2 * Math.sqrt(3e6)) / 27.5) ** 2 - 18.5 ** 2) * 1.3);
  });

  it('is asked for in engine coordinates', () => {
    const g = exposureGrid(box, -18.5, 10);
    expect(g.n % 2).toBe(1);
    expect(g.x0).toBe(-10);
    expect(g.x0 + (g.n - 1) * g.step).toBeCloseTo(10);
    expect(exposureSpec(g, 'magnetic', 20)).toMatchObject({ kind: 'magnetic', powerW: 1, z: { start: 1.5, step: 0, count: 1 } });
  });
});

describe('exposure map', () => {
  const grid = { x0: -2, y0: -2, step: 1, n: 5, z: 0, center: { x: 0, y: 0 } };
  /** |F| = size(x, y) along X at every point. */
  const field = (size: (x: number, y: number) => number): Sample[] => {
    const out: Sample[] = [];
    for (let iy = 0; iy < 5; iy++) {
      for (let ix = 0; ix < 5; ix++) {
        const at = { x: ix - 2, y: iy - 2, z: 0, magnitude: 0, phaseDeg: 0 };
        out.push({ ...at, component: 'X', real: 0, imag: size(at.x, at.y) });
        out.push({ ...at, component: 'Y', real: 0, imag: 0 });
        out.push({ ...at, component: 'Z', real: 0, imag: 0 });
      }
    }
    return out;
  };
  const none = new Float32Array(25);
  const lim = limits(145);

  it('scales the field at 1 W to the power and compares it with the limit', () => {
    // 2.75 V/m at 1 W is 27.5 V/m at 100 W: the limit, at the middle only
    const e = field((x, y) => (x === 0 && y === 0 ? 2.75 : 0.275));
    const m = exposureMap(e, field(() => 0), grid, none, settings, lim)!;
    expect(m.maxE).toBeCloseTo(27.5);
    expect(m.maxRatio).toBeCloseTo(1);
    expect(m.ratio[12]).toBeCloseTo(1);
    expect(m.ratio[0]).toBeCloseTo(0.1);
    expect(m.exceedReach).toBe(0);
    expect(m.exceedsEdge).toBe(false);
  });

  it('takes the magnetic field against its own limit', () => {
    const m = exposureMap(field(() => 0), field(() => 0.0146), grid, none, settings, lim)!;
    expect(m.maxH).toBeCloseTo(0.146);
    expect(m.maxRatio).toBeCloseTo(0.146 / 0.0728);
    expect(m.exceedsEdge).toBe(true);
    expect(m.exceedReach).toBeCloseTo(Math.hypot(2, 2));
  });

  it('judges the instantaneous limit at the peak power below 10 MHz', () => {
    // 7 MHz: 824/7 ≈ 118 V/m averaged, 83 V/m instantaneous
    const e = field(() => 1);
    const avg = exposureMap(e, field(() => 0), grid, none, { ...settings, averageW: 2500, peakW: 2500 }, limits(7))!;
    expect(avg.maxRatio).toBeCloseTo(50 / 83);
    const ssb = exposureMap(e, field(() => 0), grid, none, { ...settings, averageW: 400, peakW: 10000 }, limits(7))!;
    expect(ssb.maxRatio).toBeCloseTo(100 / 83);
    // a peak below the average is taken as the average
    const low = exposureMap(e, field(() => 0), grid, none, { ...settings, averageW: 10000, peakW: 100 }, limits(7))!;
    expect(low.maxRatio).toBeCloseTo(100 / 83);
  });

  it('leaves points too close to a wire out of the maximum but not out of the reach', () => {
    const unsure = new Float32Array(25);
    unsure[24] = 1;
    const e = field((x, y) => (x === 2 && y === 2 ? 100 : 0.1));
    const m = exposureMap(e, field(() => 0), grid, unsure, settings, lim)!;
    expect(m.maxE).toBeCloseTo(1);
    expect(m.exceedReach).toBeCloseTo(Math.hypot(2, 2));
  });

  it('marks points within a segment of a wire', () => {
    const wire = { wire: 1, start: { x: -2, y: 0, z: 0 }, end: { x: 2, y: 0, z: 0 }, radius: 0.001, segments: 8 };
    const u = unsurePoints(grid, [wire]);
    // the row y = 0 is on the wire; a segment is 0.5 m, the next rows are 1 m away
    expect(Array.from(u.slice(10, 15))).toEqual([1, 1, 1, 1, 1]);
    expect(u.reduce((a, b) => a + b, 0)).toBe(5);
  });

  it('gives nothing for samples not of the grid', () => {
    expect(exposureMap(field(() => 1).slice(3), field(() => 0), grid, none, settings, lim)).toBeNull();
  });
});
