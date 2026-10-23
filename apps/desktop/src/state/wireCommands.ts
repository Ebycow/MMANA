// Edits of the selected wires shared by the 3D view, its toolbar and the
// keyboard shortcuts. Each is one undoable step.
import { notifications } from '@mantine/notifications';
import type { Point3, Wire } from '../api/types';
import {
  alignToOrigin,
  appendCopies,
  connectedWires,
  mirrorCopy,
  mirrorInPlace,
  removeWires,
  rotateSelection,
  rotation,
  selectionCenter,
  transformWires,
  translation,
  unitAxis,
  type Axis,
  type Edited,
} from '../domain/wireEdit';
import type { FeedRef } from '../domain/feed';
import { fixJunctions, joinTouched, type JunctionIssue, type Split } from '../domain/junctions';
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

export const copySelected = () => {
  const sel = selection();
  if (sel.length) useEditor.getState().set({ clipboard: [...sel].sort((a, b) => a - b).map((i) => structuredClone(wires()[i])) });
};

export const cutSelected = () => {
  copySelected();
  deleteSelected();
};

export function paste() {
  const clip = useEditor.getState().clipboard;
  if (clip.length) addWires(structuredClone(clip));
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

export const flipSelected = (axis: Axis) => withSelection((sel) => commitWires(mirrorInPlace(wires(), sel, axis)))();
export const mirrorCopySelected = (axis: Axis) =>
  withSelection((sel) => {
    const r = mirrorCopy(wires(), sel, axis);
    if (r.selection.length === 0) {
      notifications.show({ color: 'yellow', title: '鏡像コピー', message: `選択したワイヤーは ${axis.toUpperCase()} = 0 の面の上にあり、鏡像は元と同じです。` });
      return;
    }
    commitEdited(r);
  })();
export const alignSelected = (axis: Axis) => withSelection((sel) => commitWires(alignToOrigin(wires(), sel, axis, follow())))();
export const rotateSelected = (axis: Axis, deg: number) =>
  withSelection((sel) => commitWires(rotateSelection(wires(), sel, axis, deg, follow())))();

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
