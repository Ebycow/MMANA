import { describe, expect, it } from 'vitest';
import type { Point3, Wire } from '../api/types';
import { newProject, newWire } from '../domain/project';
import {
  addedWire,
  alignToOrigin,
  appendCopies,
  connectedWires,
  facingPlane,
  followWire,
  insertWires,
  mirrorCopy,
  mirrorInPlace,
  moveEndpoint,
  moveWires,
  niceStep,
  polygonPoints,
  polylineWires,
  rectanglePoints,
  removeWires,
  rotateSelection,
  segmentHitsRect,
  snapAngle,
  snapToGrid,
  transformWires,
  translation,
} from '../domain/wireEdit';

const p = (x: number, y: number, z: number): Point3 => ({ x, y, z });
const w = (a: Point3, b: Point3): Wire => newWire(a, b);

// a square loop in the YZ plane plus a separate dipole
const square = () => [
  w(p(0, 0, 0), p(0, 1, 0)),
  w(p(0, 1, 0), p(0, 1, 1)),
  w(p(0, 1, 1), p(0, 0, 1)),
  w(p(0, 0, 1), p(0, 0, 0)),
  w(p(2, -1, 0), p(2, 1, 0)),
];

describe('transforms of a selection', () => {
  it('moves the selection and, with follow, the ends that meet it', () => {
    const moved = transformWires(square(), [1], translation(p(0, 0.5, 0)), true);
    expect(moved[1].start).toEqual(p(0, 1.5, 0));
    expect(moved[0].end).toEqual(p(0, 1.5, 0));
    expect(moved[2].start).toEqual(p(0, 1.5, 1));
    expect(moved[0].start).toEqual(p(0, 0, 0));
    expect(moved[4]).toEqual(square()[4]);
  });

  it('leaves other wires alone without follow', () => {
    const moved = transformWires(square(), [1], translation(p(0, 0.5, 0)), false);
    expect(moved[0].end).toEqual(p(0, 1, 0));
  });

  it('rotates about the selection centre without rounding noise', () => {
    const r = rotateSelection(square(), [4], 'z', 90, false);
    expect(r[4].start).toEqual(p(3, 0, 0));
    expect(r[4].end).toEqual(p(1, 0, 0));
  });

  it('moves one end and the ends that meet it', () => {
    const m = moveEndpoint(square(), 0, 'end', p(0, 2, 0), true);
    expect(m[0].end).toEqual(p(0, 2, 0));
    expect(m[1].start).toEqual(p(0, 2, 0));
    expect(moveEndpoint(square(), 0, 'end', p(0, 2, 0), false)[1].start).toEqual(p(0, 1, 0));
  });

  it('aligns the centre of the selection to the origin', () => {
    const a = alignToOrigin(square(), [4], 'x', false);
    expect(a[4].start.x).toBe(0);
    expect(a[0]).toEqual(square()[0]);
  });
});

describe('mirror', () => {
  it('flips in place about the selection centre', () => {
    const m = mirrorInPlace([w(p(1, 0, 0), p(3, 0, 1))], [0], 'x');
    expect(m[0].start).toEqual(p(3, 0, 0));
    expect(m[0].end).toEqual(p(1, 0, 1));
  });

  it('copies in the plane through the origin and skips wires already there', () => {
    const half = [w(p(0, 0, 0), p(0, 1, 0)), w(p(0, 0, 0), p(0, 0, 1))];
    const r = mirrorCopy(half, [0, 1], 'y');
    // the second wire lies on y = 0, its image is itself
    expect(r.wires).toHaveLength(3);
    expect(r.selection).toEqual([2]);
    expect(r.wires[2].end).toEqual(p(0, -1, 0));
  });
});

describe('deleting wires', () => {
  it('renumbers pulses and removes those on deleted wires', () => {
    const project = newProject();
    const a = project.antenna;
    a.wires = square();
    a.sources = [
      { pulse: 'w5c', phaseDeg: 0, voltage: 0.5 },
      { pulse: 'w2b', phaseDeg: 0, voltage: 0.5 },
    ];
    a.loads = [{ pulse: 'ｗ４ｅ', kind: 'impedance', r: 50, x: 0 }];
    const r = removeWires(a, [1, 2]);
    expect(r.antenna.wires).toHaveLength(3);
    expect(r.antenna.sources.map((s) => s.pulse)).toEqual(['w3c']);
    expect(r.antenna.sources[0].voltage).toBe(1);
    expect(r.antenna.loads[0].pulse).toBe('w2e');
    expect(r.warnings).toHaveLength(1);
  });

  it('keeps raw pulse numbers with a warning', () => {
    expect(followWire('12', [0, null])).toMatchObject({ pulse: '12', keep: true });
    expect(followWire('12', [0, null]).warning).toBeDefined();
  });
});

describe('shapes', () => {
  it('draws a polyline, dropping zero-length steps', () => {
    const like = { ...newWire(p(0, 0, 0), p(0, 0, 0)), radius: 0.002 };
    const ws = polylineWires([p(0, 0, 0), p(1, 0, 0), p(1, 0, 0), p(1, 1, 0)], like);
    expect(ws).toHaveLength(2);
    expect(ws[1].radius).toBe(0.002);
  });

  it('closes rectangles and polygons exactly', () => {
    const r = rectanglePoints(p(0, 0, 5), p(2, 1, 5), 'xy');
    expect(r).toEqual([p(0, 0, 5), p(2, 0, 5), p(2, 1, 5), p(0, 1, 5), p(0, 0, 5)]);
    const g = polygonPoints(p(0, 0, 0), p(0, 1, 0), 'yz', 4);
    expect(g).toHaveLength(5);
    expect(g[1]).toEqual(p(0, 0, 1));
    expect(g[4]).toBe(g[0]);
  });

  it('finds the wires of a loop', () => {
    expect(connectedWires(square(), 2)).toEqual([0, 1, 2, 3]);
    expect(connectedWires(square(), 4)).toEqual([4]);
  });
});

describe('snapping', () => {
  it('snaps to the grid within the plane', () => {
    expect(snapToGrid(p(0.26, 0.74, 0.33), 'xy', 0.25)).toEqual(p(0.25, 0.75, 0.33));
  });
  it('snaps directions to angle steps', () => {
    const s = snapAngle(p(0, 0, 0), p(1, 0.1, 0), 'xy', 15);
    expect(s.y).toBe(0);
    expect(s.x).toBeCloseTo(Math.hypot(1, 0.1));
  });
  it('picks round grid steps', () => {
    expect(niceStep(0.23)).toBe(0.2);
    expect(niceStep(0.04)).toBe(0.05);
    expect(niceStep(8)).toBe(10);
  });
  it('hits rectangles with segments that cross them', () => {
    const r = { x0: 0, y0: 0, x1: 10, y1: 10 };
    expect(segmentHitsRect({ x: -5, y: 5 }, { x: 15, y: 5 }, r)).toBe(true);
    expect(segmentHitsRect({ x: -5, y: -5 }, { x: -1, y: 20 }, r)).toBe(false);
  });
});

describe('inserting wires', () => {
  it('renumbers the pulses of the wires after the insertion', () => {
    const a = newProject().antenna;
    a.wires = square();
    a.sources = [{ pulse: 'w2c', phaseDeg: 0, voltage: 1 }];
    a.loads = [{ pulse: 'w1c', kind: 'impedance', r: 50, x: 0 }];
    const r = insertWires(a, 1, [w(p(5, 0, 0), p(5, 1, 0))]);
    expect(r.antenna.wires).toHaveLength(6);
    expect(r.antenna.wires[1].start).toEqual(p(5, 0, 0));
    expect(r.antenna.sources[0].pulse).toBe('w3c');
    expect(r.antenna.loads[0].pulse).toBe('w1c');
  });
});

describe('the wire the list adds', () => {
  const settings = { mode: 'zero' as const, offset: p(0.1, 0, 0), start: p(1, 2, 3), end: p(1, 2, 4) };
  const last: Wire = { ...w(p(0, -1, 0), p(0, 1, 0)), radius: 0.002, note: 'dipole', group: 'DE' };

  it('starts at the origin with zero length, like MMANA', () => {
    const n = addedWire(last, settings);
    expect([n.start, n.end]).toEqual([p(0, 0, 0), p(0, 0, 0)]);
    expect(n).toMatchObject({ radius: 0.002, note: '', group: 'DE' });
  });

  it('continues from the end of the last wire', () => {
    const n = addedWire(last, { ...settings, mode: 'continue' });
    expect([n.start, n.end]).toEqual([p(0, 1, 0), p(0, 2, 0)]);
  });

  it('shifts the last wire, sharing no end with it', () => {
    const n = addedWire(last, { ...settings, mode: 'offset' });
    expect([n.start, n.end]).toEqual([p(0.1, -1, 0), p(0.1, 1, 0)]);
  });

  it('uses the fixed coordinates', () => {
    const n = addedWire(last, { ...settings, mode: 'fixed' });
    expect([n.start, n.end]).toEqual([p(1, 2, 3), p(1, 2, 4)]);
  });

  it('starts a 1 m wire at the origin when there is no last wire to follow', () => {
    for (const mode of ['continue', 'offset'] as const) {
      const n = addedWire(undefined, { ...settings, mode });
      expect([n.start, n.end]).toEqual([p(0, 0, 0), p(0, 1, 0)]);
    }
    expect(addedWire(undefined, settings)).toEqual(newWire(p(0, 0, 0), p(0, 0, 0)));
  });

  it('does not share the segmentation object with the last wire', () => {
    expect(addedWire(last, settings).segmentation).not.toBe(last.segmentation);
  });
});

describe('moving wires', () => {
  const antenna = () => {
    const a = newProject().antenna;
    a.wires = square();
    a.sources = [{ pulse: 'w5c', phaseDeg: 0, voltage: 1 }];
    a.loads = [{ pulse: 'w1b', kind: 'impedance', r: 50, x: 0 }];
    return a;
  };

  it('moves the selection into a gap in its order and renumbers pulses', () => {
    const a = antenna();
    // wires 5 and 1 into the gap before wire 3
    const r = moveWires(a, [4, 0], 2);
    expect(r.antenna.wires).toEqual([a.wires[1], a.wires[0], a.wires[4], a.wires[2], a.wires[3]]);
    expect(r.moved).toEqual([1, 0, 3, 4, 2]);
    expect(r.selection).toEqual([1, 2]);
    expect(r.antenna.sources[0].pulse).toBe('w3c');
    expect(r.antenna.loads[0].pulse).toBe('w2b');
    expect(r.warnings).toEqual([]);
  });

  it('moves to the end and to the start', () => {
    const a = antenna();
    expect(moveWires(a, [0], 5).antenna.wires.at(-1)).toBe(a.wires[0]);
    expect(moveWires(a, [4], 0).antenna.wires[0]).toBe(a.wires[4]);
  });

  it('leaves the order alone when the gap is next to the selection', () => {
    const a = antenna();
    expect(moveWires(a, [1, 2], 1).antenna.wires).toEqual(a.wires);
    expect(moveWires(a, [1, 2], 3).antenna.wires).toEqual(a.wires);
    expect(moveWires(a, [1, 2], 2).antenna.sources).toEqual(a.sources);
  });

  it('puts the moved wires in a group, or takes them out of it', () => {
    const a = antenna();
    const r = moveWires(a, [0, 1], 5, 'Loop');
    expect(r.antenna.wires.slice(3).map((x) => x.group)).toEqual(['Loop', 'Loop']);
    expect(r.antenna.wires.slice(0, 3).every((x) => x.group === undefined)).toBe(true);
    const back = moveWires(r.antenna, [3], 0, '');
    expect('group' in back.antenna.wires[0]).toBe(false);
  });
});

describe('copies made in place', () => {
  // the loop and a copy of it on top (wires 5-8)
  const withCopy = () => {
    const ws = square().slice(0, 4);
    return appendCopies(ws, [0, 1, 2, 3], 1, (q) => q, false).wires;
  };

  it('move without their originals, which only overlap them', () => {
    const moved = transformWires(withCopy(), [4, 5, 6, 7], translation(p(1, 0, 0)), true);
    expect(moved.slice(0, 4)).toEqual(square().slice(0, 4));
    expect(moved[4].start).toEqual(p(1, 0, 0));
  });

  it('drag an end without the originals but with the joined copy', () => {
    const m = moveEndpoint(withCopy(), 4, 'end', p(0, 2, 0), true, [4, 5, 6, 7]);
    expect(m[5].start).toEqual(p(0, 2, 0));
    expect(m[0].end).toEqual(p(0, 1, 0));
    expect(m[1].start).toEqual(p(0, 1, 0));
  });

  it('still drag a wire that joins the selection at both ends', () => {
    // wire 4 bridges the two ends of a selected wire's neighbours
    const ws = [w(p(0, 0, 0), p(1, 0, 0)), w(p(1, 0, 0), p(1, 1, 0)), w(p(0, 0, 0), p(1, 1, 0))];
    const moved = transformWires(ws, [0, 1], translation(p(0, 0, 1)), true);
    expect(moved[2].start).toEqual(p(0, 0, 1));
    expect(moved[2].end).toEqual(p(1, 1, 1));
  });
});

describe('facing plane', () => {
  const look = (x: number, y: number, z: number) => {
    const n = Math.hypot(x, y, z);
    return p(x / n, y / n, z / n);
  };
  it('draws on the ground unless the view is nearly level', () => {
    expect(facingPlane(look(1, -1.15, -0.75), null)).toBe('xy');
    expect(facingPlane(look(0, 1, -0.1), null)).toBe('xz');
    expect(facingPlane(look(1, 0, -0.1), null)).toBe('yz');
  });
  it('keeps the plane in use near a boundary', () => {
    // about 19° down: level by the threshold, but not far enough to leave the ground plane
    const dz = look(1, -1, -0.49);
    expect(facingPlane(dz, null)).toBe('xz');
    expect(facingPlane(dz, 'xy')).toBe('xy');
    expect(facingPlane(look(1, -1, -0.2), 'xy')).toBe('xz');
    // a level view at 48° from X stays on the plane it had
    const diag = look(1, -1.11, 0);
    expect(facingPlane(diag, 'yz')).toBe('yz');
    expect(facingPlane(diag, 'xz')).toBe('xz');
    expect(facingPlane(look(1, -1.5, 0), 'yz')).toBe('xz');
    // and climbs back to the ground only clearly above the threshold
    expect(facingPlane(look(1, -1, -0.6), 'xz')).toBe('xz');
    expect(facingPlane(look(1, -1, -0.8), 'xz')).toBe('xy');
  });
});
