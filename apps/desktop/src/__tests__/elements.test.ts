import { describe, expect, it } from 'vitest';
import type { ElementRow } from '../api/types';
import {
  acceptsValue,
  cellOp,
  cellValue,
  elementsCsv,
  lengthFromDisplay,
  project,
  radiusFromDisplay,
  radiusToDisplay,
  replaceOps,
  strDbl,
} from '../domain/elements';

const row = (patch: Partial<ElementRow>): ElementRow => ({
  kind: 'horizontalDipole',
  interval: 2.6,
  position: 0,
  width: 10.9,
  height: 0,
  perimeter: 0,
  xWidth: true,
  radius: 0.0125,
  segmentation: { kind: 'taper' },
  wireCount: 1,
  ...patch,
});

// 3EL20: reflector, radiator (base), director
const rows = [
  row({}),
  row({ interval: 0, position: 2.6, width: 10.63 }),
  row({ position: 5.2, width: 10, radius: null, segmentation: { kind: 'taperCustom', dm1: 400, dm2: 40 } }),
];
const metres = { wavelengths: false, lambda: 21.338 };
const lambdas = { wavelengths: true, lambda: 20 };

describe('element editor helpers', () => {
  it('formats like StrDbl', () => {
    expect(strDbl(10.63)).toBe('10.63');
    expect(strDbl(2)).toBe('2.0');
    expect(strDbl(0.0000004)).toBe('0.0');
    expect(strDbl(-1.25)).toBe('-1.25');
  });

  it('converts display units like SetRmdVal / SetRmdValD', () => {
    expect(lengthFromDisplay(0.5, lambdas)).toBe(10);
    expect(lengthFromDisplay(0.5, metres)).toBe(0.5);
    expect(radiusFromDisplay(12.5, metres)).toBe(0.0125);
    expect(radiusFromDisplay(0.001, lambdas)).toBe(0.02);
    expect(radiusToDisplay(0.02, lambdas)).toBeCloseTo(0.001, 12);
  });

  it('shows text where the table has no value', () => {
    expect(cellValue(rows, 1, 1, 'interval', metres)).toBeNull();
    expect(cellValue(rows, 1, 2, 'radius', metres)).toBeNull();
    expect(cellValue(rows, 1, 0, 'radius', metres)).toBe(12.5);
    expect(cellValue(rows, 1, 2, 'position', lambdas)).toBeCloseTo(0.26, 12);
  });

  it('turns cell input into edits in metres', () => {
    expect(cellOp(3, 'width', 0.5, lambdas, true)).toEqual({ op: 'setWidth', row: 3, value: 10, scaleAll: true });
    expect(cellOp(0, 'radius', 12.5, metres, false)).toEqual({ op: 'setRadius', row: 0, value: 0.0125 });
    expect(acceptsValue('width', 0.0009)).toBe(false);
    expect(acceptsValue('interval', 0)).toBe(true);
    expect(acceptsValue('position', -1)).toBe(false);
  });

  it('replaces values as the table shows them', () => {
    const ops = replaceOps(rows, 1, 'interval', 2.6, 2.8, metres, false);
    expect(ops).toEqual([
      { op: 'setInterval', row: 0, value: 2.8 },
      { op: 'setInterval', row: 2, value: 2.8 },
    ]);
    // a replacement the cell refuses (below 0.001 in the shown unit) does nothing
    expect(replaceOps(rows, 1, 'width', 10.63, 0.0005, metres, false)).toEqual([]);
    expect(replaceOps(rows, 1, 'width', 0.5315, 0.002, lambdas, false)).toEqual([
      { op: 'setWidth', row: 1, value: 0.04, scaleAll: false },
    ]);
    const seg = replaceOps(rows, 1, 'segmentation', { kind: 'taper' }, { kind: 'fixed', count: 21 }, metres, false);
    expect(seg.map((o) => o.op === 'setSegmentation' && o.row)).toEqual([0, 1]);
  });

  it('writes the table as CSV with both positions and intervals', () => {
    const lines = elementsCsv(rows, 1, metres).trimEnd().split('\r\n');
    expect(lines[0]).toBe('No.,形式,位置(m),間隔(m),横幅(m),縦幅(m),Ｘ幅(m),R(mm),Seg.,ワイヤ数');
    expect(lines[2]).toBe('2,水平DP,2.6,基準位置,10.63,0.0,0.0,12.5,-1,1');
    expect(lines[3]).toBe('3,水平DP,5.2,2.6,10.0,0.0,0.0,複合,"400,40",1');
  });

  it('projects: front view shows Y to the right and Z up, top view shows X up', () => {
    const p = { x: 1, y: 2, z: 3 };
    expect(project(p, { azimuth: 0, elevation: 0 }).map((v) => +v.toFixed(12))).toEqual([2, 3]);
    expect(project(p, { azimuth: 0, elevation: 90 }).map((v) => +v.toFixed(12))).toEqual([2, -1]);
  });
});
