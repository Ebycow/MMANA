import { describe, expect, it } from 'vitest';
import type { Antenna, Point3, Wire } from '../api/types';
import { findJunctionIssues, fixJunctions, isMistake, joinTouched, splitWires } from '../domain/junctions';
import { newProject, newWire } from '../domain/project';

const p = (x: number, y: number, z: number): Point3 => ({ x, y, z });
const w = (a: Point3, b: Point3): Wire => newWire(a, b);

const antenna = (wires: Wire[], sources: string[] = [], loads: string[] = []): Antenna => {
  const a = newProject().antenna;
  a.wires = wires;
  a.sources = sources.map((pulse) => ({ pulse, phaseDeg: 0, voltage: 1 / sources.length }));
  a.loads = loads.map((pulse) => ({ pulse, kind: 'impedance', r: 50, x: 0 }));
  return a;
};

// a dipole along Y and a vertical wire standing on its middle
const tee = () => [w(p(0, -5, 10), p(0, 5, 10)), w(p(0, 0, 10), p(0, 0, 12))];

describe('finding junctions the engine does not make', () => {
  it('finds an end on the middle of a wire', () => {
    const issues = findJunctionIssues(tee());
    expect(issues).toEqual([{ kind: 'tee', wire: 0, by: { wire: 1, end: 'start' }, at: p(0, 0, 10) }]);
  });

  it('finds ends that touch but are not the same point', () => {
    const ws = [w(p(0, 0, 10), p(0, 1, 10)), w(p(0, 1.0005, 10), p(0, 2, 10))];
    const [issue] = findJunctionIssues(ws);
    expect(issue).toMatchObject({ kind: 'gap', at: p(0, 1, 10) });
    expect(issue.kind === 'gap' && issue.distance).toBeCloseTo(0.0005);
  });

  it('treats ends a hair apart as a gap, though the view shows them joined', () => {
    const ws = [w(p(0, 0, 10), p(0, 1, 10)), w(p(0, 1 + 1e-9, 10), p(0, 2, 10))];
    expect(findJunctionIssues(ws).map((i) => i.kind)).toEqual(['gap']);
  });

  it('leaves ends apart by more than the radii alone', () => {
    const ws = [w(p(0, 0, 10), p(0, 1, 10)), w(p(0, 1.01, 10), p(0, 2, 10))];
    expect(findJunctionIssues(ws)).toEqual([]);
  });

  it('reports crossings, but not as mistakes', () => {
    const ws = [w(p(0, -1, 10), p(0, 1, 10)), w(p(-1, 0, 10), p(1, 0, 10))];
    const issues = findJunctionIssues(ws);
    expect(issues).toEqual([{ kind: 'cross', wires: [0, 1], at: p(0, 0, 10) }]);
    expect(issues.filter(isMistake)).toEqual([]);
  });

  it('reports overlapping wires', () => {
    const ws = [w(p(0, 0, 10), p(0, 1, 10)), w(p(0, 1, 10), p(0, 0, 10))];
    expect(findJunctionIssues(ws)).toEqual([{ kind: 'overlap', wires: [0, 1] }]);
  });

  it('is quiet on joined wires and loops', () => {
    const loop = [w(p(0, 0, 0), p(0, 1, 0)), w(p(0, 1, 0), p(0, 1, 1)), w(p(0, 1, 1), p(0, 0, 1)), w(p(0, 0, 1), p(0, 0, 0))];
    expect(findJunctionIssues(loop)).toEqual([]);
    expect(findJunctionIssues(newProject().antenna.wires)).toEqual([]);
  });
});

describe('splitting wires', () => {
  it('cuts a wire in place and renumbers the pulses after it', () => {
    const a = antenna([w(p(0, 0, 0), p(0, 4, 0)), w(p(1, 0, 0), p(1, 1, 0))], ['w2c']);
    const r = splitWires(a, [{ wire: 0, at: p(0, 1, 0) }, { wire: 0, at: p(0, 3, 0) }]);
    expect(r.antenna.wires.map((x) => [x.start.y, x.end.y])).toEqual([
      [0, 1],
      [1, 3],
      [3, 4],
      [0, 1],
    ]);
    expect(r.pieces).toEqual([[0, 1, 2], [3]]);
    expect(r.antenna.sources[0].pulse).toBe('w4c');
  });

  it('names the centre of a wire cut in half as the end of its first half', () => {
    const r = splitWires(antenna(tee(), ['w1c']), [{ wire: 0, at: p(0, 0, 10) }]);
    expect(r.antenna.sources[0].pulse).toBe('w1e');
    expect(r.warnings).toHaveLength(1);
  });

  it('keeps pulses counted from an end on the piece at that end', () => {
    const r = splitWires(antenna(tee(), ['w1b3'], ['ｗ１ｅ２']), [{ wire: 0, at: p(0, 1, 10) }]);
    expect(r.antenna.sources[0].pulse).toBe('w1b3');
    expect(r.antenna.loads[0].pulse).toBe('w2e2');
    expect(r.warnings).toEqual([]);
  });

  it('keeps a centre that stays the centre of a piece, and warns when it moves', () => {
    const three = splitWires(antenna(tee(), ['w1c']), [{ wire: 0, at: p(0, -1, 10) }, { wire: 0, at: p(0, 1, 10) }]);
    expect(three.antenna.sources[0].pulse).toBe('w2c');
    expect(three.warnings).toEqual([]);
    const off = splitWires(antenna(tee(), ['w1c']), [{ wire: 0, at: p(0, 2, 10) }]);
    expect(off.antenna.sources[0].pulse).toBe('w1c');
    expect(off.warnings).toHaveLength(1);
  });

  it('shares out a fixed segment count', () => {
    const ws = tee();
    ws[0].segmentation = { kind: 'fixed', count: 21 };
    const r = splitWires(antenna(ws), [{ wire: 0, at: p(0, 0, 10) }]);
    expect(r.antenna.wires.slice(0, 2).map((x) => x.segmentation)).toEqual([
      { kind: 'fixed', count: 11 },
      { kind: 'fixed', count: 11 },
    ]);
  });

  it('does not cut combined wires or at an end', () => {
    const ws = tee();
    ws[0].radius = -1;
    expect(splitWires(antenna(ws), [{ wire: 0, at: p(0, 0, 10) }]).antenna.wires).toHaveLength(2);
    expect(splitWires(antenna(tee()), [{ wire: 0, at: p(0, 5, 10) }]).antenna.wires).toHaveLength(2);
  });
});

describe('joining new wires', () => {
  it('cuts the wire a new one starts from', () => {
    const r = joinTouched(antenna(tee()), [1]);
    expect(r.antenna.wires).toHaveLength(3);
    expect(r.pieces).toEqual([[0, 1], [2]]);
    expect(findJunctionIssues(r.antenna.wires)).toEqual([]);
  });

  it('cuts a new wire drawn through an existing end', () => {
    // the vertical wire is drawn first; the dipole is the new one
    const r = joinTouched(antenna([tee()[1], tee()[0]]), [1]);
    expect(r.antenna.wires).toHaveLength(3);
    expect(r.pieces).toEqual([[0], [1, 2]]);
  });

  it('leaves a copy made in place alone', () => {
    const ws = [...tee(), structuredClone(tee()[1])];
    expect(joinTouched(antenna(ws), [2]).antenna.wires).toHaveLength(3);
  });
});

describe('fixing junctions', () => {
  it('fixes every mistake in one go, leaving nothing to report', () => {
    const ws = [
      ...tee(),
      w(p(0, 5.0003, 10), p(0, 6, 10)), // a gap at the dipole's end
      w(p(3, 0, 0), p(3, 1, 0)),
      w(p(3, 0, 0), p(3, 1, 0)), // an overlapping copy
    ];
    const a = antenna(ws, ['w1c', 'w5b']);
    const issues = findJunctionIssues(ws);
    expect(issues.map((i) => i.kind).sort()).toEqual(['gap', 'overlap', 'tee']);
    const r = fixJunctions(a, issues);
    expect(findJunctionIssues(r.antenna.wires)).toEqual([]);
    expect(r.antenna.wires).toHaveLength(5);
    expect(r.pieces).toEqual([[0, 1], [2], [3], [4], []]);
    expect(r.antenna.wires[3].start).toEqual(p(0, 5, 10));
    // the centre feed is now the junction; the dropped copy's source is on the wire it lay on
    expect(r.antenna.sources.map((s) => s.pulse)).toEqual(['w1e', 'w5b']);
  });

  it('moves an end that nearly touches a wire onto it', () => {
    const ws = [w(p(0, -5, 10), p(0, 5, 10)), w(p(0, 0, 10.0004), p(0, 0, 12))];
    const r = fixJunctions(antenna(ws), findJunctionIssues(ws));
    expect(r.antenna.wires[2].start).toEqual(p(0, 0, 10));
    expect(findJunctionIssues(r.antenna.wires)).toEqual([]);
  });

  it('joins crossing wires at their crossing when asked', () => {
    const ws = [w(p(0, -1, 10), p(0, 1, 10)), w(p(-1, 0, 10), p(1, 0, 10))];
    const r = fixJunctions(antenna(ws), findJunctionIssues(ws));
    expect(r.antenna.wires).toHaveLength(4);
    expect(findJunctionIssues(r.antenna.wires)).toEqual([]);
  });

  it('sends the pulses of a reversed copy to the matching end', () => {
    const ws = [w(p(0, 0, 10), p(0, 1, 10)), w(p(0, 1, 10), p(0, 0, 10))];
    const r = fixJunctions(antenna(ws, ['w2b2']), findJunctionIssues(ws));
    expect(r.antenna.sources[0].pulse).toBe('w1e2');
  });
});
