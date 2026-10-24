import { describe, expect, it } from 'vitest';
import type { Wire } from '../api/types';
import { newProject, newWire } from '../domain/project';
import { groupMembers, groupNames, groupWires, listRows, newGroupName, renameGroup, ungroupWires } from '../domain/wireGroups';

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
