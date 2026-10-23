import { describe, expect, it } from 'vitest';
import { niceTicks } from '../charts/ticks';
import { radiusFor, sampleAngle, sampleAt } from '../charts/polar';
import { evaluate } from '../domain/expr';
import { bounds, displayWires, normalizePulse, pulseLocation, reachingGround, sceneGrid } from '../domain/geometry';
import { UNTITLED, newProject, newSource, setAutoVoltage, suggestedFileName } from '../domain/project';
import { formatSegmentation, parseSegmentation, segmentationFromCode, segmentationToCode } from '../domain/segmentation';
import { sweepOrder } from '../domain/sweep';

describe('segmentation codes (must match mmana-core Segmentation::to_code/from_code)', () => {
  it.each([11, 0, -1, -2, -3, -5])('round-trips %i', (code) => {
    expect(segmentationToCode(segmentationFromCode(code))).toBe(code);
  });
  it('packs dm1,dm2 like MMANA (0x80000000 | dm1 | dm2 << 16)', () => {
    const code = segmentationToCode({ kind: 'taperCustom', dm1: 400, dm2: 40 });
    expect(code).toBe((0x80000000 | 400 | (40 << 16)) | 0);
    expect(segmentationFromCode(code)).toEqual({ kind: 'taperCustom', dm1: 400, dm2: 40 });
  });
  it('parses and formats the SEG column', () => {
    expect(parseSegmentation('400,40')).toEqual({ kind: 'taperCustom', dm1: 400, dm2: 40 });
    expect(parseSegmentation('40,400')).toBeNull();
    expect(parseSegmentation('-1')).toEqual({ kind: 'taper' });
    expect(parseSegmentation('abc')).toBeNull();
    expect(formatSegmentation({ kind: 'taperCustom', dm1: 400, dm2: 40 })).toBe('400,40');
    expect(formatSegmentation({ kind: 'fixed', count: 11 })).toBe('11');
  });
});

describe('expression input', () => {
  const ctx = { wavelength: 20 };
  it('evaluates arithmetic and units', () => {
    expect(evaluate('1+2*3', ctx)).toBe(7);
    expect(evaluate('R/4', ctx)).toBe(5);
    expect(evaluate('-(1.5+0.5)*2', ctx)).toBe(-4);
    expect(evaluate('8e-4', ctx)).toBe(0.0008);
    expect(evaluate('２＊I', ctx)).toBeCloseTo(0.0508);
  });
  it('rejects invalid input', () => {
    expect(evaluate('', ctx)).toBeNull();
    expect(evaluate('1+', ctx)).toBeNull();
    expect(evaluate('(1', ctx)).toBeNull();
    expect(evaluate('1/0', ctx)).toBeNull();
  });
});

describe('polar geometry (MMANA orientation)', () => {
  it('scales by 10^(dB/40) relative to the maximum', () => {
    expect(radiusFor(10, 10)).toBe(1);
    expect(radiusFor(-30, 10)).toBeCloseTo(0.1);
  });
  it('spaces dB evenly on the linear-dB scale', () => {
    expect(radiusFor(10, 10, 'linearDb')).toBe(1);
    expect(radiusFor(-10, 10, 'linearDb')).toBeCloseTo(0.5);
    expect(radiusFor(-999, 10, 'linearDb')).toBe(0);
  });
  it('puts horizontal azimuth 0 at the top, clockwise', () => {
    expect(sampleAngle('horizontal', 0, 1)).toBeCloseTo(Math.PI / 2);
    expect(sampleAngle('horizontal', 90, 1)).toBeCloseTo(0); // 90° → right
    expect(sampleAt('horizontal', 10, 0, 1, 360)).toBe(90);
    expect(sampleAt('horizontal', 0, -10, 1, 360)).toBe(0);
  });
  it('puts the vertical front horizon right and zenith on top', () => {
    expect(sampleAngle('vertical', 180, 1)).toBeCloseTo(0);
    expect(sampleAngle('vertical', 90, 1)).toBeCloseTo(Math.PI / 2);
    expect(sampleAt('vertical', 0, -10, 1, 181)).toBe(90);
    expect(sampleAt('vertical', 0, 10, 1, 181)).toBeNull();
  });
});

describe('sweep order', () => {
  it('starts with the ends, quarters and centre, then fills the widest gaps', () => {
    const order = sweepOrder(21);
    expect(order.slice(0, 5)).toEqual([0, 5, 10, 15, 20]);
    expect(order.slice(5, 9)).toEqual([2, 7, 12, 17]);
  });
  it.each([1, 2, 3, 4, 5, 6, 21, 100, 401])('visits each of %i points once', (n) => {
    expect([...sweepOrder(n)].sort((a, b) => a - b)).toEqual(Array.from({ length: n }, (_, i) => i));
  });
});

describe('axis ticks', () => {
  it('produces round steps covering the range', () => {
    expect(niceTicks(13.9, 14.4, 5)).toEqual([13.9, 14, 14.1, 14.2, 14.3, 14.4]);
    const t = niceTicks(1.03, 2.37, 4);
    expect(t[0]).toBeLessThanOrEqual(1.03);
    expect(t.at(-1)).toBeGreaterThanOrEqual(2.37);
  });
});

describe('3D view geometry', () => {
  it('expands stacks like mmana-core (horizontal along Y, centred)', () => {
    const p = newProject();
    p.antenna.stack = { ...p.antenna.stack, horizontal: 2, horizontalSpacing: 10 };
    const w = displayWires(p.antenna);
    expect(w).toHaveLength(2);
    expect(w[0].start.y - p.antenna.wires[0].start.y).toBe(-5);
    expect(w[1].start.y - p.antenna.wires[0].start.y).toBe(5);
    expect(w.map((x) => x.base)).toEqual([0, 0]);
  });
  it('locates pulse names on their wire', () => {
    const p = newProject();
    const c = pulseLocation('w1c', p.antenna.wires)!;
    expect(c.y).toBeCloseTo(0);
    expect(pulseLocation('w9c', p.antenna.wires)).toBeNull();
    expect(pulseLocation('ｗ１ｃ', p.antenna.wires)).toEqual(c);
  });
  it('converts full-width pulse names to half-width', () => {
    expect(normalizePulse(' ｗ１ｃ　')).toBe('w1c');
    expect(normalizePulse('Ｗ３Ｂ－１')).toBe('W3B-1');
  });
  it('spaces the grid by the snap step under the antenna', () => {
    // a 2 m dipole off the origin, in free space
    const box = bounds([{ base: 0, start: { x: 0.33, y: -1, z: 0 }, end: { x: 0.33, y: 1, z: 0 }, radius: 0.001 }]);
    expect(box.maxZ).toBe(0);
    expect(sceneGrid(box, 0, 0.1)).toEqual({ center: { x: 0.33, y: 0 }, z: 0, half: 3, cell: 0.1, section: 1 });
  });
  it('frames the ground below the antenna', () => {
    // a 6 m horizontal dipole 8 m up
    const box = bounds([{ base: 0, start: { x: 0, y: -3, z: 0 }, end: { x: 0, y: 3, z: 0 }, radius: 0.001 }]);
    const f = reachingGround(box, -8);
    expect(f.center).toEqual({ x: 0, y: 0, z: -4 });
    // 5 m to the corners, with the margin
    expect(f.radius).toBeCloseTo(6.25);
    expect([f.minZ, f.maxZ]).toEqual([-8, 0]);
  });
  it('keeps the spacing when the ground is far below', () => {
    const box = bounds([{ base: 0, start: { x: 0, y: -1, z: 0 }, end: { x: 0, y: 1, z: 0 }, radius: 0.001 }]);
    const g = sceneGrid(box, -10, 0.05);
    // 480 cells of 5 cm across: the shader leaves out the ones too fine to see
    expect(g.cell).toBe(0.05);
    expect(g.section).toBe(0.5);
    expect(g.half).toBe(12);
    expect(g.z).toBe(-10);
  });
  it('keeps a grid spacing chosen by the viewer', () => {
    const box = bounds([{ base: 0, start: { x: 0, y: -1, z: 0 }, end: { x: 0, y: 1, z: 0 }, radius: 0.001 }]);
    const g = sceneGrid(box, -10, 0.1);
    expect([g.cell, g.section]).toEqual([0.1, 1]);
  });
});

describe('automatic source voltage (SetAutoVol)', () => {
  const withSources = (count: number, autoVoltage: boolean) => {
    const { antenna } = newProject();
    antenna.autoVoltage = autoVoltage;
    antenna.sources = Array.from({ length: count }, () => ({ ...newSource(), voltage: 1 / count }));
    return antenna;
  };

  it('spreads 1 V over the sources after adding or removing one', () => {
    const antenna = withSources(2, true);
    antenna.sources.push(newSource());
    setAutoVoltage(antenna);
    expect(antenna.sources.map((s) => s.voltage)).toEqual([1 / 3, 1 / 3, 1 / 3]);
    antenna.sources.splice(0, 2);
    setAutoVoltage(antenna);
    expect(antenna.sources.map((s) => s.voltage)).toEqual([1]);
  });

  it('keeps the voltages when the setting is off', () => {
    const antenna = withSources(2, false);
    antenna.sources.push(newSource());
    setAutoVoltage(antenna);
    expect(antenna.sources.map((s) => s.voltage)).toEqual([0.5, 0.5, 1]);
  });

  it('accepts an antenna without sources', () => {
    const antenna = withSources(0, true);
    setAutoVoltage(antenna);
    expect(antenna.sources).toEqual([]);
  });
});

describe('the file name offered for an untitled antenna', () => {
  it('replaces the characters file systems refuse', () => {
    expect(suggestedFileName('3el 20m: 2/3λ "test"')).toBe('3el 20m_ 2_3λ _test_.maa');
  });

  it('drops trailing dots and spaces, which Windows strips', () => {
    expect(suggestedFileName('  yagi.. ')).toBe('yagi.maa');
  });

  it('falls back to untitled for a blank name', () => {
    expect(suggestedFileName(' . ')).toBe(UNTITLED);
  });
});
