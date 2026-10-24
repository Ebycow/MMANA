import { describe, expect, it } from 'vitest';
import type { Wire } from '../api/types';
import { newProject, newWire } from '../domain/project';
import { dropTarget, groupMembers, groupNames, groupWires, listRows, newGroupName, renameGroup, ungroupWires, type Dragged } from '../domain/wireGroups';

const wire = (y: number, group?: string): Wire => ({ ...newWire({ x: 0, y, z: 0 }, { x: 0, y: y + 1, z: 0 }), ...(group && { group }) });

describe('the rows of the wire list', () => {
  it('puts a heading over each run of wires in a group', () => {
    const rows = listRows([wire(0), wire(1, 'A'), wire(2, 'A'), wire(3, 'B'), wire(4), wire(5, 'A')]);
    expect(rows).toEqual([
      { kind: 'wire', wire: 0, group: '' },
      { kind: 'group', name: 'A', wires: [1, 2] },
      { kind: 'wire', wire: 1, group: 'A' },
      { kind: 'wire', wire: 2, group: 'A' },
      { kind: 'group', name: 'B', wires: [3] },
      { kind: 'wire', wire: 3, group: 'B' },
      { kind: 'wire', wire: 4, group: '' },
      { kind: 'group', name: 'A', wires: [5] },
      { kind: 'wire', wire: 5, group: 'A' },
    ]);
  });

  it('finds the members and names of groups', () => {
    const ws = [wire(0, 'B'), wire(1), wire(2, 'A'), wire(3, 'B')];
    expect(groupMembers(ws, 'B')).toEqual([0, 3]);
    expect(groupMembers(ws, '')).toEqual([]);
    expect(groupNames(ws)).toEqual(['B', 'A']);
    expect(newGroupName([wire(0, 'グループ 1'), wire(1, 'グループ 3')])).toBe('グループ 2');
  });
});

describe('editing groups', () => {
  it('gathers the selection where the first of it is', () => {
    const a = newProject().antenna;
    a.wires = [wire(0), wire(1), wire(2), wire(3)];
    a.sources = [{ pulse: 'w4c', phaseDeg: 0, voltage: 1 }];
    const r = groupWires(a, [3, 1], 'Driven');
    expect(r.antenna.wires.map((w) => w.start.y)).toEqual([0, 1, 3, 2]);
    expect(r.antenna.wires.map((w) => w.group ?? '')).toEqual(['', 'Driven', 'Driven', '']);
    expect(r.selection).toEqual([1, 2]);
    expect(r.antenna.sources[0].pulse).toBe('w3c');
  });

  it('ungroups and renames', () => {
    const ws = [wire(0, 'A'), wire(1, 'A'), wire(2, 'B')];
    expect(ungroupWires(ws, [1]).map((w) => w.group)).toEqual(['A', undefined, 'B']);
    expect(renameGroup(ws, 'A', 'B').map((w) => w.group)).toEqual(['B', 'B', 'B']);
    expect(renameGroup(ws, 'A', '').map((w) => w.group)).toEqual([undefined, undefined, 'B']);
  });
});

describe('where a drop puts the wires', () => {
  // 1, [A: 2 3], 4
  const ws = [wire(0), wire(1, 'A'), wire(2, 'A'), wire(3)];
  const rows = listRows(ws);
  const open = () => false;
  const wiresDrag = { kind: 'wires' } as const;

  it('joins the group of the wire it is dropped next to', () => {
    // rows: 0 wire1, 1 head A, 2 wire2, 3 wire3, 4 wire4
    expect(dropTarget(rows, { row: 2, lower: false }, 4, wiresDrag, open)).toMatchObject({ at: 1, group: 'A', line: { row: 2, edge: 'top', indent: true } });
    expect(dropTarget(rows, { row: 3, lower: true }, 4, wiresDrag, open)).toMatchObject({ at: 3, group: 'A' });
    expect(dropTarget(rows, { row: 4, lower: false }, 4, wiresDrag, open)).toMatchObject({ at: 3, group: '' });
    expect(dropTarget(rows, 'end', 4, wiresDrag, open)).toMatchObject({ at: 4, group: '', line: { row: 4, edge: 'bottom' } });
  });

  it('goes before a group on the upper half of its heading and into it on the lower', () => {
    expect(dropTarget(rows, { row: 1, lower: false }, 4, wiresDrag, open)).toMatchObject({ at: 1, group: '' });
    expect(dropTarget(rows, { row: 1, lower: true }, 4, wiresDrag, open)).toMatchObject({ at: 1, group: 'A' });
    // a collapsed group takes it at its end
    expect(dropTarget(rows, { row: 1, lower: true }, 4, wiresDrag, () => true)).toMatchObject({ at: 3, group: 'A' });
  });

  it('keeps a dragged group whole and out of other groups', () => {
    const two = [wire(0, 'A'), wire(1, 'A'), wire(2, 'B'), wire(3, 'B'), wire(4)];
    const r2 = listRows(two);
    // rows: 0 head A, 1 w1, 2 w2, 3 head B, 4 w3, 5 w4, 6 w5
    const a: Dragged = { kind: 'group', name: 'A', wires: [0, 1] };
    const b: Dragged = { kind: 'group', name: 'B', wires: [2, 3] };
    // inside A: before A
    expect(dropTarget(r2, { row: 2, lower: false }, 5, b, open)).toEqual({ at: 0, group: undefined, line: { row: 0, edge: 'top', indent: false } });
    // after B
    expect(dropTarget(r2, { row: 6, lower: false }, 5, a, open)).toMatchObject({ at: 4, group: undefined });
    // into B, which it is already before
    expect(dropTarget(r2, { row: 5, lower: false }, 5, a, open)).toBeNull();
    // on itself
    expect(dropTarget(r2, { row: 2, lower: true }, 5, a, open)).toBeNull();
    expect(dropTarget(r2, { row: 0, lower: false }, 5, a, open)).toBeNull();
  });
});
