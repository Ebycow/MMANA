// Random sequences of wire commands, run through the store as the UI runs
// them. After every command the document must hold together and the undo
// history must be able to take it back; at the end, undoing everything gives
// the starting antenna and redoing everything gives the last one.
// FC_RUNS sets the number of sequences (the nightly quality run takes more).
import fc from 'fast-check';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Load, Point3, Project, Source, Wire } from '../api/types';
import { clipboardText } from '../api/transport';
import { normalizePulse } from '../domain/geometry';
import { findJunctionIssues } from '../domain/junctions';
import { newProject } from '../domain/project';
import { AXES } from '../domain/wireEdit';
import { groupNames, listRows } from '../domain/wireGroups';
import { wiresToText } from '../domain/wireText';
import { useEditor } from '../state/editor';
import { useApp, type AppState } from '../state/store';
import * as cmd from '../state/wireCommands';

const RUNS = Number(import.meta.env.FC_RUNS ?? 100);
const GROUPS = ['A', 'B', 'グループ 1'];

// mostly on a 0.5 m grid, so that ends meet and land on the middle of wires
const coordinate = fc.oneof(
  { weight: 4, arbitrary: fc.integer({ min: -4, max: 4 }).map((v) => v / 2) },
  { weight: 1, arbitrary: fc.double({ min: -5, max: 5, noNaN: true }) },
);
const point: fc.Arbitrary<Point3> = fc.record({ x: coordinate, y: coordinate, z: coordinate });

const segmentation: fc.Arbitrary<Wire['segmentation']> = fc.oneof(
  fc.integer({ min: 1, max: 21 }).map((count) => ({ kind: 'fixed' as const, count })),
  fc.constantFrom({ kind: 'auto' as const }, { kind: 'taper' as const }, { kind: 'taperStart' as const }, { kind: 'taperEnd' as const }),
  fc.record({ kind: fc.constant('taperCustom' as const), dm1: fc.integer({ min: 4, max: 800 }), dm2: fc.option(fc.integer({ min: 1, max: 80 })) }),
);

const wire: fc.Arbitrary<Wire> = fc
  .record({
    start: point,
    end: point,
    radius: fc.constantFrom(0.001, 0.0008, 0.002, 0),
    segmentation,
    note: fc.constantFrom('', '給電'),
    group: fc.constantFrom('', '', ...GROUPS),
  })
  .map(({ group, ...w }) => (group ? { ...w, group } : w));

/** A pulse on wire `n` (1-based) of `count`, chosen by `pick`. */
const pulseOf = (pick: number, count: number) => `w${(pick % count) + 1}${['c', 'b', 'e', ''][pick % 4]}`;

const start = fc
  .record({ wires: fc.array(wire, { minLength: 1, maxLength: 6 }), sources: fc.array(fc.nat(), { maxLength: 2 }), loads: fc.array(fc.nat(), { maxLength: 2 }) })
  .map(({ wires, sources, loads }) => {
    const p = newProject();
    p.antenna.wires = wires;
    p.antenna.sources = sources.map((k): Source => ({ pulse: pulseOf(k, wires.length), phaseDeg: 0, voltage: 1 }));
    p.antenna.loads = loads.map((k): Load => ({ pulse: pulseOf(k, wires.length), kind: 'impedance', r: 50, x: 0 }));
    return p;
  });

const axis = fc.constantFrom(...AXES);
const selectionCommand: fc.Arbitrary<cmd.SelectionCommand> = fc.oneof(
  fc.record({ kind: fc.constantFrom('flip' as const, 'mirrorCopy' as const, 'align' as const), axis }),
  fc.record({ kind: fc.constant('rotate' as const), axis, deg: fc.constantFrom(15, 90, 180, -45) }),
);
const transform: fc.Arbitrary<cmd.NumericTransform> = fc.oneof(
  fc.record({ kind: fc.constant('move' as const), offset: point, copies: fc.integer({ min: 0, max: 2 }) }),
  fc.record({
    kind: fc.constant('rotate' as const),
    axis,
    deg: fc.constantFrom(30, 90, 120, -60),
    about: fc.constantFrom('selection' as const, 'origin' as const),
    copies: fc.integer({ min: 0, max: 2 }),
  }),
);

const command = fc.oneof(
  fc.record({ k: fc.constant('select' as const), picks: fc.array(fc.nat(), { maxLength: 4 }) }),
  fc.record({ k: fc.constant('toggle' as const), pick: fc.nat() }),
  fc.record({ k: fc.constant('selectAll' as const) }),
  fc.record({ k: fc.constant('selectConnected' as const), pick: fc.nat(), add: fc.boolean() }),
  fc.record({ k: fc.constant('add' as const), wires: fc.array(wire, { minLength: 1, maxLength: 3 }) }),
  fc.record({ k: fc.constant('delete' as const) }),
  fc.record({ k: fc.constant('duplicate' as const) }),
  fc.record({ k: fc.constant('copy' as const) }),
  fc.record({ k: fc.constant('cut' as const) }),
  fc.record({ k: fc.constant('paste' as const) }),
  fc.record({ k: fc.constant('pasteFromOtherWindow' as const), wires: fc.array(wire, { minLength: 1, maxLength: 3 }) }),
  fc.record({ k: fc.constant('selection' as const), c: selectionCommand }),
  fc.record({ k: fc.constant('preview' as const), c: selectionCommand }),
  fc.record({ k: fc.constant('transform' as const), t: transform }),
  fc.record({ k: fc.constant('reorder' as const), at: fc.nat(), group: fc.option(fc.constantFrom('', ...GROUPS), { nil: undefined }) }),
  fc.record({ k: fc.constant('moveGroup' as const), pick: fc.nat(), at: fc.nat() }),
  fc.record({ k: fc.constant('duplicateGroup' as const), pick: fc.nat() }),
  fc.record({ k: fc.constant('group' as const) }),
  fc.record({ k: fc.constant('ungroup' as const) }),
  fc.record({ k: fc.constant('dissolve' as const), pick: fc.nat() }),
  fc.record({ k: fc.constant('rename' as const), pick: fc.nat(), to: fc.constantFrom(' ', ...GROUPS) }),
  fc.record({ k: fc.constant('selectGroup' as const), pick: fc.nat(), add: fc.boolean() }),
  fc.record({ k: fc.constant('placeFeed' as const), load: fc.boolean(), index: fc.nat(), pick: fc.nat() }),
  fc.record({ k: fc.constant('fixIssues' as const) }),
  fc.record({ k: fc.constant('undo' as const) }),
  fc.record({ k: fc.constant('redo' as const) }),
  fc.record({ k: fc.constant('travel' as const), steps: fc.integer({ min: -3, max: 3 }) }),
  fc.record({ k: fc.constant('settings' as const), follow: fc.boolean(), autoJoin: fc.boolean() }),
);
type Command = typeof command extends fc.Arbitrary<infer C> ? C : never;

// the system clipboard as another window or app leaves it
let board = '';
beforeEach(() => {
  vi.spyOn(clipboardText, 'write').mockImplementation(async (t) => void (board = t));
  vi.spyOn(clipboardText, 'read').mockImplementation(async () => board);
});
afterEach(() => vi.restoreAllMocks());

const app = () => useApp.getState();
const wires = () => app().project.antenna.wires;
const at = (pick: number) => pick % Math.max(wires().length, 1);
const groupRuns = () => listRows(wires()).flatMap((r) => (r.kind === 'group' ? [r] : []));
const pickOf = <T,>(list: T[], pick: number): T | undefined => list[pick % Math.max(list.length, 1)];

async function run(c: Command) {
  const n = wires().length;
  switch (c.k) {
    case 'select':
      return app().setSelection(n ? c.picks.map(at) : []);
    case 'toggle':
      return n && app().toggleWire(at(c.pick));
    case 'selectAll':
      return cmd.selectAll();
    case 'selectConnected':
      return n && cmd.selectConnected(at(c.pick), c.add);
    case 'add':
      return cmd.addWires(structuredClone(c.wires));
    case 'delete':
      return cmd.deleteSelected();
    case 'duplicate':
      return cmd.duplicateSelected();
    case 'copy':
      return cmd.copySelected();
    case 'cut':
      return cmd.cutSelected();
    case 'paste':
      return cmd.paste();
    case 'pasteFromOtherWindow':
      board = wiresToText(c.wires);
      return cmd.paste();
    case 'selection':
      return cmd.runSelectionCommand(c.c);
    case 'preview':
      cmd.previewSelectionCommand(c.c);
      return cmd.previewSelectionCommand(null);
    case 'transform':
      return cmd.transformSelected(c.t);
    case 'reorder':
      return cmd.reorderWires(app().selectedWires, c.at % (n + 1), c.group);
    case 'moveGroup': {
      const g = pickOf(groupRuns(), c.pick);
      return g && cmd.moveGroup(g.name, g.wires, c.at % (n + 1));
    }
    case 'duplicateGroup': {
      const g = pickOf(groupRuns(), c.pick);
      return g && cmd.duplicateWireGroup(g.name, g.wires);
    }
    case 'group':
      return cmd.groupSelected();
    case 'ungroup':
      return cmd.ungroupSelected();
    case 'dissolve': {
      const name = pickOf(groupNames(wires()), c.pick);
      return name && cmd.dissolveGroup(name);
    }
    case 'rename': {
      const name = pickOf(groupNames(wires()), c.pick);
      return name && cmd.renameWireGroup(name, c.to);
    }
    case 'selectGroup': {
      const name = pickOf(groupNames(wires()), c.pick);
      return name && cmd.selectGroup(name, c.add);
    }
    case 'placeFeed': {
      const items = c.load ? app().project.antenna.loads : app().project.antenna.sources;
      return n && items.length && cmd.placeFeed({ kind: c.load ? 'load' : 'source', index: c.index % items.length }, pulseOf(c.pick, n));
    }
    case 'fixIssues':
      return cmd.fixIssues(findJunctionIssues(wires()));
    case 'undo':
      return app().undo();
    case 'redo':
      return app().redo();
    case 'travel':
      return app().travel(c.steps);
    case 'settings':
      return useEditor.getState().set({ follow: c.follow, autoJoin: c.autoJoin });
  }
}

/** The wire number (1-based) of a pulse name, or null for a pulse number. */
const wireNumber = (pulse: string) => {
  const m = /^w(\d+)/i.exec(normalizePulse(pulse));
  return m ? Number(m[1]) : null;
};

const shape = (w: Wire) => JSON.stringify([w.start, w.end, w.radius, w.segmentation]);

/** The shape of the wire each source and load is on, and where on it: what a reordering must keep. */
function feedPlaces(p: Project, deleted: Set<number> = new Set()) {
  const place = (pulse: string) => {
    const n = wireNumber(pulse)!;
    return `${shape(p.antenna.wires[n - 1])} ${normalizePulse(pulse).replace(/^w\d+/i, '')}`;
  };
  const kept = (item: { pulse: string }) => !deleted.has(wireNumber(item.pulse)! - 1);
  return { sources: p.antenna.sources.filter(kept).map((s) => place(s.pulse)), loads: p.antenna.loads.filter(kept).map((l) => place(l.pulse)) };
}

/** Commands that move or regroup wires without changing them; `delete` also removes the feeds on deleted wires. */
const KEEPS_FEEDS = new Set<Command['k']>(['reorder', 'moveGroup', 'duplicateGroup', 'group', 'ungroup', 'dissolve', 'rename', 'delete']);

function holdsTogether(where: string) {
  const s = app();
  const n = s.project.antenna.wires.length;
  for (const w of s.project.antenna.wires) {
    for (const v of [w.start.x, w.start.y, w.start.z, w.end.x, w.end.y, w.end.z, w.radius]) expect(Number.isFinite(v), `${where}: a wire value is ${v}`).toBe(true);
  }
  expect(new Set(s.selectedWires).size, `${where}: a wire is selected twice`).toBe(s.selectedWires.length);
  for (const i of s.selectedWires) expect(i >= 0 && i < n, `${where}: selected wire ${i} of ${n}`).toBe(true);
  if (s.selectedWire != null) expect(s.selectedWire < n, `${where}: current wire ${s.selectedWire} of ${n}`).toBe(true);
  for (const item of [...s.project.antenna.sources, ...s.project.antenna.loads]) {
    const k = wireNumber(item.pulse);
    if (k != null) expect(k >= 1 && k <= n, `${where}: ${item.pulse} with ${n} wires`).toBe(true);
  }
}

const isRun = (indices: number[]) => indices.every((i, k) => k === 0 || i === indices[k - 1] + 1);

/** What each edit is for, given the state before it (`s0`) and after it (`s1`). */
function doesWhatItSays(c: Command, s0: AppState, s1: AppState, where: string) {
  const w0 = s0.project.antenna.wires;
  const w1 = s1.project.antenna.wires;
  const sel = [...s0.selectedWires].sort((a, b) => a - b);
  const renaming = useEditor.getState().renamingGroup;
  switch (c.k) {
    case 'add':
      expect(w1.length, where).toBeGreaterThanOrEqual(w0.length + c.wires.length);
      break;
    case 'delete':
      expect(w1.length, where).toBe(w0.length - sel.length);
      expect(s1.selectedWires, where).toEqual([]);
      break;
    case 'duplicate':
      // the copies lie on the originals, so automatic joining leaves them whole
      expect(w1.length, where).toBe(w0.length + sel.length);
      expect(s1.selectedWires.map((i) => shape(w1[i])), `${where}: the copies`).toEqual(sel.map((i) => shape(w0[i])));
      break;
    case 'reorder':
    case 'moveGroup':
      expect(w1.map(shape).sort(), `${where}: the same wires`).toEqual(w0.map(shape).sort());
      break;
    case 'group': {
      const members = [...s1.selectedWires].sort((a, b) => a - b);
      expect(members.map((i) => w1[i].group), `${where}: the new group`).toEqual(members.map(() => renaming));
      expect(isRun(members), `${where}: the group is gathered`).toBe(true);
      expect(members[0], `${where}: the group is gathered where its first wire was`).toBe(sel[0]);
      break;
    }
    case 'ungroup':
      for (const i of sel) expect(w1[i].group ?? '', where).toBe('');
      break;
    case 'duplicateGroup': {
      const run = pickOf(listRows(w0).flatMap((r) => (r.kind === 'group' ? [r.wires] : [])), c.pick)!;
      const copies = run.map((_, k) => run.at(-1)! + 1 + k);
      expect(s1.selectedWires, `${where}: the copies come right after the group`).toEqual(copies);
      expect(copies.map((i) => shape(w1[i])), `${where}: the copies`).toEqual(run.map((i) => shape(w0[i])));
      expect(copies.map((i) => w1[i].group), `${where}: the copies' group`).toEqual(copies.map(() => renaming));
      break;
    }
    case 'rename': {
      const from = pickOf(groupNames(w0), c.pick)!;
      const count = (ws: Wire[], g: string) => ws.filter((w) => w.group === g).length;
      expect(count(w1, from), `${where}: wires left in the old group`).toBe(0);
      expect(count(w1, c.to), where).toBe(count(w0, from) + count(w0, c.to));
      break;
    }
  }
}

async function check(project: Project, commands: Command[]) {
  app().load({ project: structuredClone(project), warnings: [] }, '無題.maa', null);
  useEditor.getState().set({ clipboard: [], clipboardText: '', follow: true, autoJoin: true, preview: null });
  board = '';
  holdsTogether('start');
  for (const [step, c] of commands.entries()) {
    const where = `step ${step} (${c.k})`;
    const state = app();
    const { past, future } = state;
    const before = state.project;
    const copy = structuredClone(before);
    const deleted = new Set(state.selectedWires);
    const feeds = KEEPS_FEEDS.has(c.k) ? feedPlaces(before, c.k === 'delete' ? deleted : undefined) : null;

    await run(c);

    const after = app();
    holdsTogether(where);
    expect(before, `${where}: changed the project in place`).toEqual(copy);
    if (after.project === before) {
      expect(after.past.length, `${where}: a step without a change`).toBe(past.length);
      continue;
    }
    if (c.k === 'travel') continue;
    if (c.k === 'undo') {
      expect(after.future[0]?.project, `${where}: the undo lost the project it left`).toBe(before);
      expect(after.future.length).toBe(future.length + 1);
    } else {
      // an edit or a redo: the step before it holds the project it left
      expect(after.past.at(-1)?.project, `${where}: the history lost the project before it`).toBe(before);
      expect(after.past.length).toBe(past.length + 1);
      if (c.k !== 'redo') expect(after.project, `${where}: an edit that changes nothing`).not.toEqual(copy);
    }
    if (feeds) expect(feedPlaces(after.project), `${where}: a source or load moved to another wire`).toEqual(feeds);
    doesWhatItSays(c, state, after, where);
  }

  const last = structuredClone(app().project);
  const steps = app().past.length;
  app().travel(-steps);
  expect(app().project, 'undoing every step').toEqual(project);
  app().travel(steps);
  expect(app().project, 'redoing every step').toEqual(last);
}

describe('random wire commands', () => {
  it(
    'keep the document whole, and the history takes each of them back',
    async () => {
      await fc.assert(
        // size 'max': otherwise fast-check keeps arrays to about 10, whatever maxLength says
        fc.asyncProperty(start, fc.array(command, { minLength: 1, maxLength: 40, size: 'max' }), (project, commands) => check(project, commands)),
        { numRuns: RUNS },
      );
    },
    Math.max(30_000, RUNS * 100),
  );
});
