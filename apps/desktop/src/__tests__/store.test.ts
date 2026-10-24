import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CalcResult, ParsedProject, ResonanceSearch, SweepRequest, SweepResponse } from '../api/types';
import { files, transport } from '../api/transport';
import { newProject } from '../domain/project';
import { useEditor } from '../state/editor';
import { useApp } from '../state/store';
import { previewSelectionCommand, runSelectionCommand, type SelectionCommand } from '../state/wireCommands';

/** A promise the test resolves by hand, to finish a request at a chosen moment. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

const result = (gainDbi: number) => ({ gainDbi, warnings: [] }) as unknown as CalcResult;
const sweepOf = (gainDbi: number) => ({ points: [{ frequencyMhz: 14, gainDbi }] }) as unknown as SweepResponse;
const parsed = (name: string) => {
  const project = newProject();
  project.antenna.name = name;
  return { project, warnings: [] } as unknown as ParsedProject;
};
const moveWire = () => useApp.getState().edit((p) => void (p.antenna.wires[0].end.z += 1));

// the dipole the page opens with (新規作成 starts empty)
beforeEach(() => useApp.getState().load({ project: newProject(), warnings: [] }, '無題.maa', null));
afterEach(() => vi.restoreAllMocks());

describe('resonance search', () => {
  const found = { resonance: null, minMhz: 7, maxMhz: 28, calculations: 6 } as ResonanceSearch;

  it('searches from the calculation frequency and goes stale with an edit', async () => {
    const spy = vi.spyOn(transport, 'findResonance').mockResolvedValue(found);
    useApp.getState().setOptions({ frequencyMhz: 14.2 });
    await useApp.getState().findResonance();
    expect(spy.mock.calls[0][0].options?.frequencyMhz).toBe(14.2);
    expect(useApp.getState().resonance).toMatchObject({ search: found, running: false, stale: false });
    moveWire();
    expect(useApp.getState().resonance.stale).toBe(true);
  });

  it('drops a search of the previous document', async () => {
    const d = deferred<ResonanceSearch>();
    vi.spyOn(transport, 'findResonance').mockReturnValue(d.promise);
    const run = useApp.getState().findResonance();
    useApp.getState().load(parsed('other'), 'other.maa', null);
    d.resolve(found);
    await run;
    expect(useApp.getState().resonance.search).toBeNull();
  });
});

describe('sweep staleness', () => {
  it('marks the sweep stale when the antenna or options change', async () => {
    vi.spyOn(transport, 'sweep').mockResolvedValue(sweepOf(2));
    await useApp.getState().runSweep();
    expect(useApp.getState().sweep.stale).toBe(false);

    moveWire();
    expect(useApp.getState().sweep.stale).toBe(true);
    await useApp.getState().runSweep();
    expect(useApp.getState().sweep.stale).toBe(false);

    useApp.getState().setOptions({ frequencyMhz: 14.2 });
    expect(useApp.getState().sweep.stale).toBe(true);
    await useApp.getState().runSweep();
    useApp.getState().undo();
    expect(useApp.getState().sweep.stale).toBe(true);
  });

  it('keeps the sweep current when only the range changes', async () => {
    vi.spyOn(transport, 'sweep').mockResolvedValue(sweepOf(2));
    await useApp.getState().runSweep();
    useApp.getState().setSweepRange({ points: 41 });
    expect(useApp.getState().sweep.stale).toBe(false);
  });

  it('marks a sweep stale when the antenna changed while it ran', async () => {
    const d = deferred<SweepResponse>();
    vi.spyOn(transport, 'sweep').mockReturnValue(d.promise);
    const run = useApp.getState().runSweep();
    moveWire();
    d.resolve(sweepOf(2));
    await run;
    expect(useApp.getState().sweep.data).not.toBeNull();
    expect(useApp.getState().sweep.stale).toBe(true);
  });

  it('a new project starts without stale flags', async () => {
    vi.spyOn(transport, 'sweep').mockResolvedValue(sweepOf(2));
    await useApp.getState().runSweep();
    moveWire();
    useApp.getState().load(parsed('B'), 'B.maa', null);
    expect(useApp.getState().sweep).toMatchObject({ data: null, stale: false });
  });
});

describe('新規作成', () => {
  it("starts empty like MMANA's, keeping the ground and resetting the SWR reference", () => {
    useApp.getState().edit((p) => {
      p.environment.ground = 'perfect';
      p.environment.height = 7;
      p.environment.swrReference = { r: 75, x: 0 };
    });
    useApp.getState().reset();
    const { antenna, environment } = useApp.getState().project;
    expect(antenna).toMatchObject({ name: '', wires: [], sources: [], frequencyMhz: 14.05 });
    expect(environment).toMatchObject({ ground: 'perfect', height: 7, swrReference: { r: 50, x: 0 } });
    expect(useApp.getState().dirty).toBe(false);
  });
});

describe('previewing a selection command', () => {
  afterEach(() => useEditor.getState().set({ preview: null }));

  it('shows the result without editing, and running it gives the same wires', () => {
    const c: SelectionCommand = { kind: 'rotate', axis: 'z', deg: 90 };
    const before = useApp.getState().project;
    useApp.getState().selectWire(0);
    previewSelectionCommand(c);
    const shown = useEditor.getState().preview;
    expect(shown?.[0].start.x).toBeCloseTo(5.121);
    expect(useApp.getState().project).toBe(before);
    previewSelectionCommand(null);
    expect(useEditor.getState().preview).toBeNull();
    runSelectionCommand(c);
    expect(useApp.getState().project.antenna.wires).toEqual(shown);
  });

  it('shows the copies a mirror copy would add', () => {
    useApp.getState().edit((p) => void (p.antenna.wires[0] = { ...p.antenna.wires[0], start: { x: 1, y: -1, z: 0 }, end: { x: 1, y: 1, z: 0 } }));
    useApp.getState().selectWire(0);
    previewSelectionCommand({ kind: 'mirrorCopy', axis: 'x' });
    expect(useEditor.getState().preview?.map((w) => w.start.x)).toEqual([1, -1]);
    expect(useApp.getState().project.antenna.wires).toHaveLength(1);
  });

  it('shows nothing without a selection', () => {
    useApp.getState().selectWire(null);
    previewSelectionCommand({ kind: 'flip', axis: 'y' });
    expect(useEditor.getState().preview).toBeNull();
  });
});

describe('progressive sweep', () => {
  /** Answers each batch with one point per index, at its index as the frequency. */
  const pointsOf = (req: SweepRequest) =>
    ({ points: (req.indices ?? []).map((i) => ({ frequencyMhz: i, gainDbi: 0 })) }) as unknown as SweepResponse;

  it('shows the outline first, then fills in the rest in frequency order', async () => {
    const batches: (number[] | null | undefined)[] = [];
    const first = deferred<SweepResponse>();
    vi.spyOn(transport, 'sweep').mockImplementation((req) => {
      batches.push(req.indices);
      return batches.length === 1 ? first.promise : Promise.resolve(pointsOf(req));
    });
    useApp.getState().setSweepRange({ points: 21 });
    const run = useApp.getState().runSweep();
    expect(batches[0]).toEqual([0, 5, 10, 15, 20]);
    expect(useApp.getState().sweep).toMatchObject({ running: true, progress: { done: 0, total: 21 } });
    first.resolve(pointsOf({ indices: [0, 5, 10, 15, 20] } as SweepRequest));
    await run;
    const s = useApp.getState().sweep;
    expect(s.data?.map((p) => p.frequencyMhz)).toEqual(Array.from({ length: 21 }, (_, i) => i));
    expect(s).toMatchObject({ running: false, progress: null });
  });

  it('stops after the points being calculated arrive and keeps them', async () => {
    const first = deferred<SweepResponse>();
    const sweep = vi.spyOn(transport, 'sweep').mockReturnValueOnce(first.promise);
    useApp.getState().setSweepRange({ points: 21 });
    const run = useApp.getState().runSweep();
    useApp.getState().stopSweep();
    expect(useApp.getState().sweep.stopping).toBe(true);
    first.resolve(pointsOf({ indices: [0, 5, 10, 15, 20] } as SweepRequest));
    await run;
    expect(sweep).toHaveBeenCalledTimes(1);
    expect(useApp.getState().sweep).toMatchObject({ running: false, stopping: false, progress: { done: 5, total: 21 } });
    expect(useApp.getState().sweep.data).toHaveLength(5);
  });
});

describe('results from a previous document', () => {
  it('drops a calculation that finishes after another file was opened', async () => {
    const d = deferred<CalcResult>();
    vi.spyOn(transport, 'calculate').mockReturnValue(d.promise);
    const calc = useApp.getState().calculate();
    useApp.getState().load(parsed('B'), 'B.maa', null);
    d.resolve(result(7.2));
    await calc;
    const s = useApp.getState();
    expect(s.project.antenna.name).toBe('B');
    expect(s.result).toBeNull();
    expect(s.calculating).toBe(false);
  });

  it('drops a calculation error from the previous file', async () => {
    const d = deferred<CalcResult>();
    vi.spyOn(transport, 'calculate').mockReturnValue(d.promise.then(() => Promise.reject(new Error('old'))));
    const calc = useApp.getState().calculate();
    useApp.getState().load(parsed('B'), 'B.maa', null);
    d.resolve(result(0));
    await calc;
    expect(useApp.getState().calcError).toBeNull();
  });

  it('runs a calculation requested for the new file once the old one finishes', async () => {
    const first = deferred<CalcResult>();
    const spy = vi.spyOn(transport, 'calculate').mockReturnValueOnce(first.promise).mockResolvedValueOnce(result(6.8));
    const calc = useApp.getState().calculate();
    useApp.getState().load(parsed('B'), 'B.maa', null);
    void useApp.getState().calculate(); // queued, as App does after opening with auto-calc
    first.resolve(result(7.2));
    await calc;
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[1][0].project.antenna.name).toBe('B');
    expect(useApp.getState().result?.gainDbi).toBe(6.8);
    expect(useApp.getState().resultStale).toBe(false);
  });

  it('does not run a calculation queued for the previous file', async () => {
    const first = deferred<CalcResult>();
    const spy = vi.spyOn(transport, 'calculate').mockReturnValue(first.promise);
    const calc = useApp.getState().calculate();
    void useApp.getState().calculate(); // queued for the old file
    useApp.getState().load(parsed('B'), 'B.maa', null);
    first.resolve(result(7.2));
    await calc;
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('drops a sweep that finishes after another file was opened', async () => {
    const d = deferred<SweepResponse>();
    vi.spyOn(transport, 'sweep').mockReturnValue(d.promise);
    const run = useApp.getState().runSweep();
    useApp.getState().load(parsed('B'), 'B.maa', null);
    d.resolve(sweepOf(2));
    await run;
    expect(useApp.getState().sweep).toMatchObject({ data: null, running: false });
  });

  it('keeps the newer sweep when an earlier one finishes last', async () => {
    const older = deferred<SweepResponse>();
    vi.spyOn(transport, 'sweep').mockReturnValueOnce(older.promise).mockResolvedValueOnce(sweepOf(5));
    const first = useApp.getState().runSweep();
    await useApp.getState().runSweep();
    older.resolve(sweepOf(1));
    await first;
    expect(useApp.getState().sweep.data?.[0].gainDbi).toBe(5);
  });
});

describe('saving', () => {
  type Saved = { path: string | null } | null;

  it('clears the unsaved mark when nothing changed while saving', async () => {
    vi.spyOn(files, 'save').mockResolvedValue({ path: '/tmp/A.maa' });
    moveWire();
    expect(await useApp.getState().save(false)).toBe(true);
    expect(useApp.getState()).toMatchObject({ dirty: false, fileName: 'A.maa', filePath: '/tmp/A.maa' });
  });

  it('keeps the unsaved mark for an edit made while saving', async () => {
    const d = deferred<Saved>();
    const spy = vi.spyOn(files, 'save').mockReturnValue(d.promise);
    moveWire();
    const saving = useApp.getState().save(false);
    moveWire();
    d.resolve({ path: '/tmp/A.maa' });
    expect(await saving).toBe(true);
    expect(spy.mock.calls[0][0]).not.toBe(useApp.getState().project);
    expect(useApp.getState()).toMatchObject({ dirty: true, filePath: '/tmp/A.maa' });
  });

  it('leaves a file opened while saving alone', async () => {
    const d = deferred<Saved>();
    vi.spyOn(files, 'save').mockReturnValue(d.promise);
    moveWire();
    const saving = useApp.getState().save(true);
    useApp.getState().load(parsed('B'), 'B.maa', '/tmp/B.maa');
    moveWire();
    d.resolve({ path: '/tmp/A.maa' });
    await saving;
    expect(useApp.getState()).toMatchObject({ dirty: true, fileName: 'B.maa', filePath: '/tmp/B.maa' });
  });

  it('offers an untitled antenna under its name, and an opened file under the file name', async () => {
    const spy = vi.spyOn(files, 'save').mockResolvedValue(null);
    useApp.getState().edit((p) => void (p.antenna.name = '3el 20m: test'));
    await useApp.getState().save(true);
    expect(spy.mock.calls[0][2]).toBe('3el 20m_ test.maa');
    useApp.getState().load(parsed('Other name'), 'B.maa', null);
    await useApp.getState().save(true);
    expect(spy.mock.calls[1][2]).toBe('B.maa');
  });
});

describe('wire selection', () => {
  const threeWires = () =>
    useApp.getState().edit((p) => {
      const w = p.antenna.wires[0];
      p.antenna.wires.push(structuredClone(w), structuredClone(w));
    });

  it('adds, removes and replaces wires of the selection', () => {
    threeWires();
    const s = useApp.getState();
    s.selectWire(0);
    s.toggleWire(2);
    expect(useApp.getState().selectedWires).toEqual([0, 2]);
    expect(useApp.getState().selectedWire).toBe(2);
    useApp.getState().toggleWire(2);
    expect(useApp.getState().selectedWires).toEqual([0]);
    expect(useApp.getState().selectedWire).toBe(0);
    useApp.getState().setSelection([1, 1, 2], 1);
    expect(useApp.getState().selectedWires).toEqual([1, 2]);
    expect(useApp.getState().selectedWire).toBe(1);
  });

  it('drops wires an undo removes from the selection', () => {
    threeWires();
    useApp.getState().setSelection([0, 2]);
    useApp.getState().undo();
    expect(useApp.getState().selectedWires).toEqual([0]);
    expect(useApp.getState().selectedWire).toBeNull();
  });

  it('clears the selection when another document is opened', () => {
    useApp.getState().selectWire(0);
    useApp.getState().load(parsed('other'), 'other.maa', null);
    expect(useApp.getState().selectedWires).toEqual([]);
  });
});
