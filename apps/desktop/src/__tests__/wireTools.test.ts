import { describe, expect, it } from 'vitest';
import type { Point3, Wire } from '../api/types';
import type { Units } from '../domain/elements';
import { newProject, newWire } from '../domain/project';
import { ALL_SCALE_TARGETS, frequencyForScale, replaceInWires, scaleAntenna, scaleForFrequency } from '../domain/wireTools';

const p = (x: number, y: number, z: number): Point3 => ({ x, y, z });
const w = (a: Point3, b: Point3, radius = 0.001): Wire => ({ ...newWire(a, b), radius });
const metres: Units = { wavelengths: false, millimetres: false, lambda: 299.8 / 14.05 };
const mm: Units = { ...metres, millimetres: true };

describe('scaleAntenna (アンテナのサイズ)', () => {
  const antenna = () => {
    const a = newProject().antenna;
    a.frequencyMhz = 14.05;
    a.wires = [w(p(0.5, -5.1, 10), p(0.5, 5.1, 10), 0.001), w(p(1, 2, 3), p(4, 5, 6), 0), w(p(0, 0, 0), p(0, 0, 1), -1)];
    return a;
  };

  it('scales every coordinate and positive radius by fq / tfq and moves the design frequency', () => {
    const scale = scaleForFrequency(14.05, 28.1);
    const r = scaleAntenna(antenna(), scale, 28.1, ALL_SCALE_TARGETS)!;
    expect(scale).toBe(0.5);
    expect(r.frequencyMhz).toBe(28.1);
    expect(r.wires[0]).toMatchObject({ start: p(0.25, -2.55, 5), end: p(0.25, 2.55, 5), radius: 0.0005 });
    // R 0 (disabled) and a combined wire's negative key stay as they are
    expect(r.wires[1].radius).toBe(0);
    expect(r.wires[2].radius).toBe(-1);
  });

  it('multiplies in the same order as the original, so the doubles agree exactly', () => {
    const scale = scaleForFrequency(14.05, 21.2);
    const r = scaleAntenna(antenna(), scale, 21.2, ALL_SCALE_TARGETS)!;
    expect(r.wires[0].start.y).toBe(-5.1 * (14.05 / 21.2));
    expect(frequencyForScale(14.05, scale)).toBeCloseTo(21.2, 12);
  });

  it('scales only the chosen axes, and keeps the frequency when no coordinate is scaled', () => {
    const z = scaleAntenna(antenna(), 2, 7.025, { x: false, y: false, z: true, radius: false })!;
    expect(z.wires[0]).toMatchObject({ start: p(0.5, -5.1, 20), radius: 0.001 });
    expect(z.frequencyMhz).toBe(7.025);
    const r = scaleAntenna(antenna(), 2, 7.025, { x: false, y: false, z: false, radius: true })!;
    expect(r.wires[0]).toMatchObject({ start: p(0.5, -5.1, 10), radius: 0.002 });
    expect(r.frequencyMhz).toBe(14.05);
  });

  it('leaves loads, the stack and the sources alone', () => {
    const a = antenna();
    a.loads = [{ pulse: 'w1c', kind: 'impedance', r: 5, x: 20 }];
    const r = scaleAntenna(a, 0.5, 28.1, ALL_SCALE_TARGETS)!;
    expect(r.loads).toEqual(a.loads);
    expect(r.stack).toEqual(a.stack);
    expect(r.sources).toEqual(a.sources);
  });

  it('changes nothing for a scale of 1, a non-positive one, or nothing chosen', () => {
    expect(scaleAntenna(antenna(), 1, 14.05, ALL_SCALE_TARGETS)).toBeNull();
    expect(scaleAntenna(antenna(), 0, 14.05, ALL_SCALE_TARGETS)).toBeNull();
    expect(scaleAntenna(antenna(), -2, 14.05, ALL_SCALE_TARGETS)).toBeNull();
    expect(scaleAntenna(antenna(), 2, 7, { x: false, y: false, z: false, radius: false })).toBeNull();
  });
});

describe('replaceInWires (検索と置換)', () => {
  const wires = () => [w(p(0, -5, 10), p(0, 5, 10)), w(p(5, -5, 0), p(-5, 5.0000001, 5)), w(p(2.5, 0, 0), p(2.5, 0, 0.5), 0.002)];

  it('replaces a coordinate wherever it appears, as the table shows it', () => {
    const r = replaceInWires(wires(), { kind: 'coordinate', scope: 'all', absolute: false, search: 5, replacement: 7 }, metres);
    expect(r.count).toBe(4);
    expect(r.wires[0].end.y).toBe(7);
    expect(r.wires[1].start.x).toBe(7);
    // 5.0000001 shows as 5.0 with six decimals
    expect(r.wires[1].end.y).toBe(7);
    expect(r.wires[1].end.z).toBe(7);
    expect(r.wires[1].start.y).toBe(-5);
  });

  it('with 絶対値を検索, replaces the magnitude and keeps the sign relative to the search', () => {
    const r = replaceInWires(wires(), { kind: 'coordinate', scope: 'y', absolute: true, search: 5, replacement: 7 }, metres);
    expect(r.wires[0].start.y).toBe(-7);
    expect(r.wires[0].end.y).toBe(7);
    expect(r.wires[1].start.y).toBe(-7);
    // other axes untouched
    expect(r.wires[1].start.x).toBe(5);
    const neg = replaceInWires(wires(), { kind: 'coordinate', scope: 'y', absolute: true, search: -5, replacement: 7 }, metres);
    // RepJob: sgn(d) · sgn(nd) · sgn(t) · |nd|
    expect(neg.wires[0].start.y).toBe(7);
    expect(neg.wires[0].end.y).toBe(-7);
  });

  it('works in the displayed unit', () => {
    const r = replaceInWires(wires(), { kind: 'coordinate', scope: 'z', absolute: false, search: 10000, replacement: 12000 }, mm);
    expect(r.wires[0].start.z).toBe(12);
    expect(r.count).toBe(2);
  });

  it('replaces the radius (in mm) and SEG', () => {
    const r = replaceInWires(wires(), { kind: 'radius', search: 1, replacement: 1.5 }, metres);
    expect(r.count).toBe(2);
    expect(r.wires.map((x) => x.radius)).toEqual([0.0015, 0.0015, 0.002]);
    const s = replaceInWires(wires(), { kind: 'segmentation', search: { kind: 'taper' }, replacement: { kind: 'fixed', count: 11 } }, metres);
    expect(s.count).toBe(3);
    expect(s.wires[2].segmentation).toEqual({ kind: 'fixed', count: 11 });
  });

  it('finds nothing to do when the value does not change or does not occur', () => {
    const same = wires();
    expect(replaceInWires(same, { kind: 'coordinate', scope: 'all', absolute: true, search: 5, replacement: 5 }, metres)).toEqual({ wires: same, count: 0 });
    expect(replaceInWires(same, { kind: 'coordinate', scope: 'all', absolute: true, search: 3, replacement: 4 }, metres).count).toBe(0);
  });
});
