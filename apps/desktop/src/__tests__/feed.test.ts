import { describe, expect, it } from 'vitest';
import type { Point3, Project, PulseLayout, PulsePoint } from '../api/types';
import { describePulse, feedPositions, pulseNamed, pulsesFit, pulsesOnWire, type LayoutFor } from '../domain/feed';
import { newProject, newWire } from '../domain/project';

const p = (x: number, y: number, z: number): Point3 => ({ x, y, z });
const pulse = (n: number, name: string, y: number, z = 0): PulsePoint => ({ pulse: n, name, position: p(0, y, z) });

// an inverted L: wire 1 along Y from 0 to 4, wire 2 up from its end to z = 2
function project(): Project {
  const pr = newProject();
  pr.antenna.wires = [newWire(p(0, 0, 0), p(0, 4, 0)), newWire(p(0, 4, 0), p(0, 4, 2))];
  pr.antenna.sources = [{ pulse: 'w1b1', phaseDeg: 0, voltage: 1 }];
  pr.antenna.loads = [];
  return pr;
}

// wire 1 names pulses 1-3; the joint (pulse 4) is only reachable as w2b
const layout: PulseLayout = {
  pulseCount: 6,
  wires: [
    { wire: 1, pulses: [pulse(1, 'w1b', 1), pulse(2, 'w1c', 2), pulse(3, 'w1e1', 3)] },
    { wire: 2, pulses: [pulse(4, 'w2b', 4, 0), pulse(5, 'w2c', 4, 1), pulse(6, 'w2e1', 4, 1.5)] },
  ],
  sources: [p(0, 2, 0)],
  loads: [],
};

const fitFor = (pr: Project): LayoutFor => ({ layout, project: pr, frequencyMhz: pr.antenna.frequencyMhz });

describe('pulse layout', () => {
  it('fits while the wires, segmentation, ground and frequency are the same', () => {
    const pr = project();
    const l = fitFor(pr);
    expect(pulsesFit(l, pr, pr.antenna.frequencyMhz)).toBe(true);
    // sources can change without moving the pulses
    expect(pulsesFit(l, { ...pr, antenna: { ...pr.antenna, sources: [] } }, pr.antenna.frequencyMhz)).toBe(true);
    expect(pulsesFit(l, pr, pr.antenna.frequencyMhz + 1)).toBe(false);
    expect(pulsesFit(l, { ...pr, antenna: { ...pr.antenna, wires: [...pr.antenna.wires] } }, pr.antenna.frequencyMhz)).toBe(false);
    expect(pulsesFit(l, { ...pr, environment: { ...pr.environment } }, pr.antenna.frequencyMhz)).toBe(false);
  });

  it('finds a pulse by the name it gives, however it is typed', () => {
    expect(pulseNamed(layout, 'W1C')?.pulse).toBe(2);
    expect(pulseNamed(layout, 'ｗ２ｂ')?.pulse).toBe(4);
    expect(pulseNamed(layout, 'w9c')).toBeNull();
  });

  it('lists a joint with the wire it ends, in order from the start', () => {
    const pr = project();
    expect(pulsesOnWire(layout, pr.antenna.wires, 0).map((q) => q.name)).toEqual(['w1b', 'w1c', 'w1e1', 'w2b']);
    expect(pulsesOnWire(layout, pr.antenna.wires, 1).map((q) => q.name)).toEqual(['w2b', 'w2c', 'w2e1']);
  });

  it('says how far a pulse is from both ends', () => {
    const pr = project();
    expect(describePulse(layout.wires[0].pulses[2], pr.antenna.wires[0])).toBe('w1e1（始点から 3 m・終点から 1 m）');
  });
});

describe('feedPositions', () => {
  it('uses the resolved positions while the sources are the ones laid out', () => {
    const pr = project();
    expect(feedPositions(pr.antenna, fitFor(pr)).sources).toEqual([p(0, 2, 0)]);
  });

  it('looks a changed name up in the layout', () => {
    const pr = project();
    const moved = { ...pr.antenna, sources: [{ ...pr.antenna.sources[0], pulse: 'w2c' }] };
    expect(feedPositions(moved, fitFor({ ...pr })).sources).toEqual([p(0, 4, 1)]);
  });

  it('falls back to the approximate place without a layout', () => {
    const pr = project();
    pr.antenna.sources[0].pulse = 'w1c';
    expect(feedPositions(pr.antenna, null).sources).toEqual([p(0, 2, 0)]);
  });

  it('keeps a marker at its place along a wire being dragged', () => {
    const pr = project();
    // wire 1 stretched to twice its length: the source at y = 2 (half way) goes to y = 4
    const shown = [newWire(p(0, 0, 0), p(0, 8, 0)), pr.antenna.wires[1]];
    expect(feedPositions(pr.antenna, fitFor(pr), shown).sources).toEqual([p(0, 4, 0)]);
  });
});
