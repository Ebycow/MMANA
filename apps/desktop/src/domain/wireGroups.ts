// Wire groups: a name on each wire that the wire list shows as a heading over
// the wires that share it. A group is meant to be a run of neighbouring wires;
// putting wires in a group moves them next to each other. Pure functions.
import type { Antenna, Wire } from '../api/types';
import { moveWires, withGroup, type Moved } from './wireEdit';

export const groupOf = (w: Wire) => w.group ?? '';

/** A row of the wire list: a group heading over the run of wires below it, or a wire. */
export type ListRow = { kind: 'group'; name: string; wires: number[] } | { kind: 'wire'; wire: number; group: string };

/** The rows of the list: each run of neighbouring wires with the same group gets a heading. */
export function listRows(wires: Wire[]): ListRow[] {
  const rows: ListRow[] = [];
  let head: Extract<ListRow, { kind: 'group' }> | null = null;
  wires.forEach((w, i) => {
    const g = groupOf(w);
    if (!g) head = null;
    else if (head?.name !== g) rows.push((head = { kind: 'group', name: g, wires: [] }));
    head?.wires.push(i);
    rows.push({ kind: 'wire', wire: i, group: g });
  });
  return rows;
}

/** The wires of the group, wherever they are. */
export const groupMembers = (wires: Wire[], name: string) => wires.flatMap((w, i) => (name && groupOf(w) === name ? [i] : []));

/** The names in use, in list order. */
export const groupNames = (wires: Wire[]) => [...new Set(wires.map(groupOf).filter((g) => g))];

/** 「グループ n」 with the first n not in use. */
export function newGroupName(wires: Wire[]): string {
  const used = new Set(groupNames(wires));
  let n = 1;
  while (used.has(`グループ ${n}`)) n++;
  return `グループ ${n}`;
}

/** Puts the selected wires in a group, gathered where the first of them is. */
export function groupWires(antenna: Antenna, selection: number[], name: string): Moved {
  const first = Math.min(...selection);
  return moveWires(antenna, selection, first, name);
}

/** Takes the wires out of their groups, leaving them where they are. */
export const ungroupWires = (wires: Wire[], selection: number[]): Wire[] =>
  wires.map((w, i) => (selection.includes(i) && groupOf(w) ? withGroup(w, '') : w));

/** Renames a group; the name of another group joins them. */
export const renameGroup = (wires: Wire[], from: string, to: string): Wire[] =>
  wires.map((w) => (from && groupOf(w) === from ? withGroup(w, to) : w));
