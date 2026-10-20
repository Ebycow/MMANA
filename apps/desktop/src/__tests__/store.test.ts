import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CalcResult, ParsedProject, SweepResponse } from '../api/types';
import { files, transport } from '../api/transport';
import { newProject } from '../domain/project';
import { useApp } from '../state/store';

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

beforeEach(() => useApp.getState().reset());
afterEach(() => vi.restoreAllMocks());

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
