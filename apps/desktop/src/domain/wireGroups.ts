// Wire groups: a name on each wire that the wire list shows as a heading over
// the wires that share it. A group is meant to be a run of neighbouring wires;
// putting wires in a group moves them next to each other. Pure functions.
import type { Antenna, Wire } from '../api/types';
import { insertWires, moveWires, withGroup, type Moved } from './wireEdit';

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

/** 「name のコピー」, numbered when that is in use too. */
export function copyName(wires: Wire[], name: string): string {
  const used = new Set(groupNames(wires));
  const base = `${name} のコピー`;
  let n = 1;
  while (used.has(n === 1 ? base : `${base} ${n}`)) n++;
  return n === 1 ? base : `${base} ${n}`;
}

/**
 * Copies the wires under a group heading into a new group right after them.
 * The copies lie on the originals; sources and loads stay on the originals.
 */
export function duplicateGroup(antenna: Antenna, run: number[], name: string): { antenna: Antenna; warnings: string[]; selection: number[] } {
  const at = Math.max(...run) + 1;
  const copies = run.map((i) => withGroup(structuredClone(antenna.wires[i]), name));
  return { ...insertWires(antenna, at, copies), selection: copies.map((_, k) => at + k) };
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

/** Where the pointer is during a drag: a half of a shown row, or below the last. */
export type DropSpot = { row: number; lower: boolean } | 'end';

/** What is dragged: wires (the selection), or the run of wires under a group heading. */
export type Dragged = { kind: 'wires' } | { kind: 'group'; name: string; wires: number[] };

/**
 * Where a drop puts the dragged wires: the gap before wire `at`, the group
 * they join (`undefined` keeps theirs), and the edge of the shown row the
 * line is drawn at.
 */
export interface Drop {
  at: number;
  group: string | undefined;
  line: { row: number; edge: 'top' | 'bottom'; indent: boolean };
}

/**
 * The drop for a spot in the shown rows (`collapsed` groups show only their
 * heading). Wires join the group of the wire they are dropped next to; on a
 * heading they go before the group (upper half) or into it (lower half). A
 * group is kept whole: dropped inside another it goes before that one.
 */
export function dropTarget(rows: ListRow[], spot: DropSpot, wireCount: number, dragged: Dragged, collapsed: (name: string) => boolean): Drop | null {
  if (rows.length === 0) return null;
  let d: Drop;
  if (spot === 'end') {
    d = { at: wireCount, group: '', line: { row: rows.length - 1, edge: 'bottom', indent: false } };
  } else {
    const r = rows[spot.row];
    if (!r) return null;
    const edge = spot.lower ? 'bottom' : 'top';
    if (r.kind === 'wire') {
      d = { at: r.wire + (spot.lower ? 1 : 0), group: r.group, line: { row: spot.row, edge, indent: r.group !== '' } };
    } else if (!spot.lower) {
      d = { at: r.wires[0], group: '', line: { row: spot.row, edge, indent: false } };
    } else {
      d = { at: collapsed(r.name) ? r.wires.at(-1)! + 1 : r.wires[0], group: r.name, line: { row: spot.row, edge, indent: true } };
    }
  }
  if (dragged.kind === 'wires') return d;
  const own = dragged.wires;
  if (d.group && d.group !== dragged.name) {
    // before the heading of the run the spot is in
    let h = spot === 'end' ? -1 : spot.row;
    while (h >= 0 && rows[h].kind !== 'group') h--;
    const head = rows[h];
    if (head?.kind !== 'group') return null;
    d = { at: head.wires[0], group: undefined, line: { row: h, edge: 'top', indent: false } };
  }
  if (d.at >= own[0] && d.at <= own.at(-1)! + 1) return null;
  return { ...d, group: undefined };
}
