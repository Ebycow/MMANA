// Edits of the selected wires shared by the 3D view, its toolbar and the
// keyboard shortcuts. Each is one undoable step.
import { notifications } from '@mantine/notifications';
import { clipboardText } from '../api/transport';
import type { Point3, Wire } from '../api/types';
import {
  alignToOrigin,
  appendCopies,
  connectedWires,
  mirrorCopy,
  mirrorInPlace,
  moveWires,
  removeWires,
  rotateSelection,
  rotation,
  selectionCenter,
  transformWires,
  translation,
  unitAxis,
  type Axis,
  type Edited,
  type Moved,
} from '../domain/wireEdit';
import { followWires } from '../domain/optimize';
import { copyName, duplicateGroup, groupMembers, groupWires, newGroupName, renameGroup, ungroupWires } from '../domain/wireGroups';
import type { FeedRef } from '../domain/feed';
import { fixJunctions, joinTouched, type JunctionIssue, type Split } from '../domain/junctions';
import { wiresFromText, wiresToText } from '../domain/wireText';
import { useEditor } from './editor';
import { useApp } from './store';

/**
 * Wires cannot be edited while a dialog edits a copy of the antenna or an
 * optimisation is about to write its result back.
 */
export function canEditWires(): boolean {
  const s = useApp.getState();
  return !s.elementEditorOpen && !s.optimizer.running && !s.optimizer.dialogOpen && !s.optimizer.resultsOpen;
}

const selection = () => useApp.getState().selectedWires;
const wires = () => useApp.getState().project.antenna.wires;
const follow = () => useEditor.getState().follow;

const exactly = (a: Point3, b: Point3) => a.x === b.x && a.y === b.y && a.z === b.z;

/**
 * Replaces the wires as one undoable step and selects `selected` (the same
 * wires by default). With automatic joining, new or moved ends that land on
 * the middle of a wire cut it there.
 */
export function commitWires(next: Wire[], selected?: number[]) {
  const app = useApp.getState();
  const prev = app.project.antenna.wires;
  if (next === prev) return;
  if (useEditor.getState().autoJoin) {
    const touched = next.flatMap((w, i) => (prev[i] && exactly(prev[i].start, w.start) && exactly(prev[i].end, w.end) ? [] : [i]));
    const split = joinTouched({ ...app.project.antenna, wires: next }, touched);
    if (split.pieces.some((p) => p.length > 1)) return commitSplit(split, selected ?? app.selectedWires, '分けてつなぎました');
  }
  app.edit((p) => void (p.antenna.wires = structuredClone(next)));
  if (selected) app.setSelection(selected);
}

/** Puts a split antenna in place, selecting the pieces of `selected`, and tells the user about it. */
function commitSplit(split: Split, selected: number[], what: string) {
  const app = useApp.getState();
  const cut = split.pieces.filter((p) => p.length > 1).length;
  app.edit((p) => void (p.antenna = structuredClone(split.antenna)), { label: `ワイヤー ${cut} 本を分けて接続` });
  app.setSelection(selected.flatMap((i) => split.pieces[i] ?? []));
  notifications.show({
    color: split.warnings.length ? 'yellow' : 'blue',
    title: `ワイヤー ${cut} 本を${what}`,
    message: split.warnings.length ? split.warnings.join('\n') : '途中に端が触れたワイヤーを、そこで分けて計算上もつながるようにしました。',
  });
}

/** Fixes connection problems as one undoable step. */
export function fixIssues(issues: JunctionIssue[]) {
  if (issues.length === 0 || !canEditWires()) return;
  const app = useApp.getState();
  const r = fixJunctions(app.project.antenna, issues);
  app.edit((p) => void (p.antenna = structuredClone(r.antenna)), { label: `接続を ${issues.length} か所修正` });
  app.setSelection(app.selectedWires.flatMap((i) => r.pieces[i] ?? []));
  notifications.show({
    color: r.warnings.length ? 'yellow' : 'teal',
    title: `接続を ${issues.length} か所直しました`,
    message: r.warnings.length ? r.warnings.join('\n') : '計算上もつながるようになりました。',
  });
}

/** Puts a source or load on the pulse with this name, as one undoable step. */
export function placeFeed(ref: FeedRef, name: string) {
  if (!canEditWires()) return;
  const app = useApp.getState();
  const items = ref.kind === 'source' ? app.project.antenna.sources : app.project.antenna.loads;
  if (!items[ref.index] || items[ref.index].pulse === name) return;
  app.edit((p) => {
    const list: { pulse: string }[] = ref.kind === 'source' ? p.antenna.sources : p.antenna.loads;
    list[ref.index].pulse = name;
  });
}

const commitEdited = (e: Edited) => commitWires(e.wires, e.selection);

/** Runs `f` on the selection when there is one and editing is allowed. */
const withSelection = (f: (sel: number[]) => void) => () => {
  const sel = selection();
  if (sel.length > 0 && canEditWires()) f(sel);
};

/** Appends new wires and selects them. */
export function addWires(added: Wire[]) {
  if (added.length === 0 || !canEditWires()) return;
  const n = wires().length;
  commitWires([...wires(), ...added], added.map((_, i) => n + i));
}

export const deleteSelected = withSelection((sel) => {
  const app = useApp.getState();
  const r = removeWires(app.project.antenna, sel);
  app.edit((p) => void (p.antenna = r.antenna));
  app.setSelection([]);
  if (r.warnings.length) notifications.show({ color: 'yellow', title: '給電点・負荷を更新しました', message: r.warnings.join('\n') });
});

/** Copies in place and selects them, ready to be moved (MMANA's copy and paste). */
export const duplicateSelected = withSelection((sel) => {
  commitEdited(appendCopies(wires(), sel, 1, (p) => p, false));
  useEditor.getState().setTool('move');
});

/** Copies the selection, also as `.maa` wire lines on the system clipboard for other windows. */
export const copySelected = () => {
  const sel = selection();
  if (!sel.length) return;
  const clipboard = [...sel].sort((a, b) => a - b).map((i) => structuredClone(wires()[i]));
  const text = wiresToText(clipboard);
  useEditor.getState().set({ clipboard, clipboardText: text });
  // without the system clipboard the copy still pastes in this window
  clipboardText.write(text).catch(() => {});
};

export const cutSelected = () => {
  copySelected();
  deleteSelected();
};

/**
 * Pastes wire lines from the system clipboard. This window's own copy is
 * taken whole while its text is still there, keeping notes and groups, and
 * also when the clipboard cannot be read.
 */
export async function paste() {
  if (!canEditWires()) return;
  const own = useEditor.getState();
  let text: string | null = null;
  try {
    text = await clipboardText.read();
  } catch {
    // not allowed by the browser
  }
  if (text == null || text === own.clipboardText) addWires(structuredClone(own.clipboard));
  else addWires(wiresFromText(text) ?? []);
}

export function selectAll() {
  useApp.getState().setSelection(wires().map((_, i) => i));
}

/** Selects the wires joined to `wire` (an element or a loop); adds them with `add`. */
export function selectConnected(wire: number, add: boolean) {
  const chain = connectedWires(wires(), wire);
  const app = useApp.getState();
  app.setSelection(add ? [...app.selectedWires, ...chain] : chain, wire);
}

/** A command of the selection bar's menus. */
export type SelectionCommand = { kind: 'flip' | 'mirrorCopy' | 'align'; axis: Axis } | { kind: 'rotate'; axis: Axis; deg: number };

/** The wires a command leaves and the selection after it (a mirror copy selects the copies). */
function applied(c: SelectionCommand, sel: number[]): Edited {
  const ws = wires();
  switch (c.kind) {
    case 'flip':
      return { wires: mirrorInPlace(ws, sel, c.axis), selection: sel };
    case 'mirrorCopy':
      return mirrorCopy(ws, sel, c.axis);
    case 'align':
      return { wires: alignToOrigin(ws, sel, c.axis, follow()), selection: sel };
    case 'rotate':
      return { wires: rotateSelection(ws, sel, c.axis, c.deg, follow()), selection: sel };
  }
}

export const runSelectionCommand = (c: SelectionCommand) =>
  withSelection((sel) => {
    const r = applied(c, sel);
    if (c.kind === 'mirrorCopy' && r.selection.length === 0) {
      notifications.show({ color: 'yellow', title: '鏡像コピー', message: `選択したワイヤーは ${c.axis.toUpperCase()} = 0 の面の上にあり、鏡像は元と同じです。` });
      return;
    }
    commitEdited(r);
  })();

/** Shows in the 3D view what a command would do, while its menu item is pointed at; null ends it. */
export function previewSelectionCommand(c: SelectionCommand | null) {
  const sel = selection();
  useEditor.getState().set({ preview: c && sel.length > 0 && canEditWires() ? applied(c, sel).wires : null });
}

export type NumericTransform =
  | { kind: 'move'; offset: Point3; copies: number }
  | { kind: 'rotate'; axis: Axis; deg: number; about: 'selection' | 'origin'; copies: number };

/**
 * Moves or turns the selection by exact amounts. With copies > 0 the
 * selection stays and copy k is moved k times (a linear or polar array).
 */
export const transformSelected = (t: NumericTransform) =>
  withSelection((sel) => {
    const pivot = t.kind === 'rotate' && t.about === 'selection' ? selectionCenter(wires(), sel)! : { x: 0, y: 0, z: 0 };
    const step = (k: number) =>
      t.kind === 'move'
        ? translation({ x: t.offset.x * k, y: t.offset.y * k, z: t.offset.z * k })
        : rotation(pivot, unitAxis(t.axis), t.deg * k);
    if (t.copies > 0) commitEdited(appendCopies(wires(), sel, t.copies, (p, k) => step(k)(p)));
    else commitWires(transformWires(wires(), sel, step(1), follow()));
  })();

/**
 * Puts a reordering in place as one undoable step: the optimisation sheet's
 * wire references follow, and the moved wires stay selected.
 */
function commitMoved(r: Moved, label: string) {
  const app = useApp.getState();
  if (JSON.stringify(r.antenna.wires) === JSON.stringify(app.project.antenna.wires)) return;
  const setup = followWires(app.optimizer.setup, r.moved);
  const changed = JSON.stringify(setup) !== JSON.stringify(app.optimizer.setup);
  app.edit((p) => void (p.antenna = structuredClone(r.antenna)), { label, ...(changed && { setup }) });
  app.setSelection(r.selection);
  if (r.warnings.length) notifications.show({ color: 'yellow', title: '給電点・負荷を更新しました', message: r.warnings.join('\n') });
}

/** Moves wires into the gap before wire `at`, into `group` when given ('' takes them out). */
export function reorderWires(sel: number[], at: number, group?: string) {
  if (sel.length === 0 || !canEditWires()) return;
  const r = moveWires(useApp.getState().project.antenna, sel, at, group);
  const n = sel.length;
  commitMoved(r, group ? `ワイヤー ${n} 本を「${group}」へ移動` : `ワイヤー ${n} 本を移動`);
}

/** Moves the wires under a group heading into the gap before wire `at`. */
export function moveGroup(name: string, members: number[], at: number) {
  if (members.length === 0 || !canEditWires()) return;
  commitMoved(moveWires(useApp.getState().project.antenna, members, at), `グループ「${name}」を移動`);
}

/** Copies the wires under a group heading into a new group after them, selects the copies and starts naming it. */
export function duplicateWireGroup(name: string, run: number[]) {
  if (run.length === 0 || !canEditWires()) return;
  const app = useApp.getState();
  const copy = copyName(wires(), name);
  const r = duplicateGroup(app.project.antenna, run, copy);
  app.edit((p) => void (p.antenna = structuredClone(r.antenna)), { label: `グループ「${name}」を複製` });
  app.setSelection(r.selection);
  useEditor.getState().set({ renamingGroup: copy });
  if (r.warnings.length) notifications.show({ color: 'yellow', title: '給電点・負荷を更新しました', message: r.warnings.join('\n') });
}

/** Puts the selection in a new group, gathered where its first wire is; returns the name. */
export function groupSelected(): string | null {
  const sel = selection();
  if (sel.length === 0 || !canEditWires()) return null;
  const name = newGroupName(wires());
  commitMoved(groupWires(useApp.getState().project.antenna, sel, name), `グループ「${name}」を作成`);
  useEditor.getState().set({ renamingGroup: name });
  return name;
}

/** Takes the selected wires out of their groups. */
export const ungroupSelected = withSelection((sel) => {
  if (sel.every((i) => !wires()[i]?.group)) return;
  useApp.getState().edit((p) => void (p.antenna.wires = ungroupWires(p.antenna.wires, sel)), { label: 'グループを解除' });
});

/** Takes every wire of the group out of it. */
export function dissolveGroup(name: string) {
  if (!canEditWires()) return;
  const members = groupMembers(wires(), name);
  if (members.length === 0) return;
  useApp.getState().edit((p) => void (p.antenna.wires = ungroupWires(p.antenna.wires, members)), { label: `グループ「${name}」を解除` });
}

export function renameWireGroup(from: string, to: string) {
  const name = to.trim();
  if (!name || name === from || groupMembers(wires(), from).length === 0 || !canEditWires()) return;
  useApp.getState().edit((p) => void (p.antenna.wires = renameGroup(p.antenna.wires, from, name)), { label: `グループ名を「${name}」に変更` });
}

/** Selects the wires of the group; with `add`, adds them, or takes them out when all are selected. */
export function selectGroup(name: string, add: boolean) {
  const app = useApp.getState();
  const members = groupMembers(wires(), name);
  if (!add) return app.setSelection(members, members[0] ?? null);
  const all = members.every((i) => app.selectedWires.includes(i));
  app.setSelection(all ? app.selectedWires.filter((i) => !members.includes(i)) : [...app.selectedWires, ...members]);
}
