import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CalcResult, OptimizeOutcome, Progress, Trial, Variable } from '../api/types';
import { transport } from '../api/transport';
import {
  currentTrial,
  defaultSetup,
  formatBound,
  formatLink,
  makeTarget,
  matchingJx,
  needsCombination,
  parseBound,
  parseLink,
  partIndex,
  partLabels,
  sortTrials,
  tradeOffText,
  withBound,
} from '../domain/optimize';
import { newProject } from '../domain/project';
import { useApp } from '../state/store';

describe('link cell', () => {
  it('reads and writes the forms MMANA accepts', () => {
    const values = [10, 5, -3];
    const cases: [string, string][] = [
      ['0', '0'],
      ['2', '2'],
      ['-2', '-2'],
      ['1*1.05', '1*1.05'],
      ['2-1.5', '2-1.5'],
      ['-3+1.2', '-3+1.2'],
      ['2/4', '2/4.0'],
      ['2+0', '2'],
      ['-2+0', '-2'],
      ['2*1', '2'],
      ['2*-1', '-2'],
      ['-2*1.5', '2*-1.5'],
    ];
    for (const [text, back] of cases) {
      const link = parseLink(text, 0, values);
      expect(link, text).not.toBeNull();
      expect(formatLink(link!), text).toBe(back);
    }
    expect(parseLink('x', 0, values)).toBeNull();
    expect(parseLink('2+', 0, values)).toBeNull();
    expect(parseLink('0+1', 0, values)).toBeNull();
  });

  it('takes the constant from the current values with =', () => {
    const values = [10, 4, -2];
    // value 0 = value 1 + c
    expect(parseLink('2+=', 0, values)).toEqual({ kind: 'offset', variable: 1, offset: 6 });
    // value 0 = -value 3 + c
    expect(parseLink('-3+=', 0, values)).toEqual({ kind: 'negatedOffset', variable: 2, offset: 8 });
    // value 0 = value 2 * c, and = value 2 / c
    expect(parseLink('2*=', 0, values)).toEqual({ kind: 'scaled', variable: 1, factor: 2.5 });
    expect(parseLink('2/=', 0, values)).toEqual({ kind: 'divided', variable: 1, divisor: 0.4 });
    // the product of a negative link follows the variable itself, as in MMANA
    expect(parseLink('-2*=', 0, values)).toEqual({ kind: 'scaled', variable: 1, factor: 2.5 });
    expect(parseLink('2+=', 0, [null, 4])).toBeNull();
  });
});

describe('bounds', () => {
  const v: Variable = { target: { kind: 'height' }, pitch: 1, min: 0, max: 200 };
  it('reads a value or another variable', () => {
    expect(parseBound('#3')).toEqual({ from: 2 });
    expect(parseBound('12.5')).toEqual({ value: 12.5 });
    expect(parseBound('#0')).toBeNull();
    expect(parseBound('abc')).toBeNull();
    const w = withBound(v, 'max', { from: 1 });
    expect(formatBound(w, 'max')).toBe('#2');
    expect(formatBound(withBound(w, 'max', { value: 30 }), 'max')).toBe('30.0');
    expect(formatBound(v, 'min')).toBe('0.0');
  });
});

describe('targets', () => {
  it('keeps the position and part index when the kind changes', () => {
    const t = makeTarget('element', 2, 3);
    expect(t).toEqual({ kind: 'element', element: 2, part: 'perimeter' });
    expect(partIndex(t)).toBe(3);
    expect(makeTarget('wire', 2, 8)).toEqual({ kind: 'wire', wire: 2, part: 'y1' });
    expect(makeTarget('polarWire', 0, 7)).toEqual({ kind: 'polarWire', wire: 0, quantity: 'zenith', anchor: 'end' });
    expect(partIndex(makeTarget('polarWire', 0, 7))).toBe(7);
    expect(makeTarget('height', 4, 1)).toEqual({ kind: 'height' });
    expect(partLabels('element', { xWidth: true })[3]).toBe('Ｘ幅');
    expect(partLabels('load', { loadKind: 'impedance' })).toEqual(['R', 'jX']);
  });

  it('asks about elements with two or more sizes', () => {
    const row = { kind: 'horizontalOther', interval: 0, position: 0, width: 1, height: 0, perimeter: 2, xWidth: true, radius: 0.001, segmentation: null, wireCount: 3 } as const;
    expect(needsCombination([row])).toBe(true);
    expect(needsCombination([{ ...row, perimeter: 0 }])).toBe(false);
    expect(needsCombination([{ ...row, xWidth: false }])).toBe(false);
  });
});

describe('trial list', () => {
  const trial = (score: number, jx: number, values: number[]): Trial =>
    ({ score, impedance: { re: 50, im: jx }, swr: 1, gainDbi: score, fbDb: 0, elevationDeg: 0, values, horizontalPattern: [], verticalPattern: [] }) as Trial;
  const trials = [trial(1, -3, [1.0000001]), trial(3, 1, [2]), trial(2, 2, [3])];

  it('sorts like the 順序 menu', () => {
    expect(sortTrials(trials, 'score').map((t) => t.score)).toEqual([3, 2, 1]);
    expect(sortTrials(trials, 'jx').map((t) => t.score)).toEqual([3, 2, 1]);
    expect(sortTrials(trials, 'gain').map((t) => t.score)).toEqual([3, 2, 1]);
  });

  it('finds the trial the antenna is at, to six decimals', () => {
    expect(currentTrial(trials, [1])).toBe(0);
    expect(currentTrial(trials, [3])).toBe(2);
    expect(currentTrial(trials, [9])).toBe(0);
  });

  it('describes the shares of the goals', () => {
    const s = defaultSetup();
    expect(tradeOffText(s)).toBe('Gain 33.3%　F/B 33.3%　jX 33.3%');
    expect(tradeOffText({ ...s, sweepOnly: true })).toBe('目標なし（単純スイープ）');
    expect(tradeOffText({ ...s, weights: { gain: 0, fb: 0, jx: 0, swr: 0, elevation: 0, matching: 0, current: 0 } })).toContain('電流 14.3%');
  });

  it('computes the matching jX like AutoJXClick', () => {
    expect(matchingJx(25, 50)).toBe(-25);
    expect(matchingJx(50, 50)).toBe(0);
    expect(matchingJx(50, 25)).toBeNull();
  });
});

describe('optimisation in the store', () => {
  beforeEach(() => useApp.getState().reset());
  afterEach(() => vi.restoreAllMocks());

  const outcome = (): OptimizeOutcome => {
    const project = structuredClone(useApp.getState().project);
    project.antenna.wires[0].end.y = 5.5;
    project.environment.height = 12;
    const best = { values: [5.5], score: 1 } as Trial;
    return { project, frequencyMhz: 14.2, stop: { kind: 'converged' }, calculations: 7, best, trials: [best], warnings: [] };
  };

  it('applies the result as one undoable edit and keeps the frequency', async () => {
    vi.spyOn(transport, 'calculate').mockResolvedValue({ warnings: [] } as unknown as CalcResult);
    vi.spyOn(transport, 'optimize').mockImplementation(async (_req, onProgress) => {
      onProgress({ calculation: 1, improved: true, values: [5.2] } as Progress);
      onProgress({ calculation: 2, improved: false, values: [5.3] } as Progress);
      return outcome();
    });
    const before = useApp.getState().project;
    await useApp.getState().runOptimization();
    const s = useApp.getState();
    expect(s.optimizer.running).toBe(false);
    expect(s.optimizer.log.map((p) => p.calculation)).toEqual([1, 2]);
    expect(s.optimizer.best?.calculation).toBe(1);
    expect(s.project.antenna.wires[0].end.y).toBe(5.5);
    expect(s.project.environment.height).toBe(12);
    expect(s.options.frequencyMhz).toBe(14.2);
    expect(s.optimizer.outcomeSetup).toBe(s.optimizer.setup);
    s.undo();
    expect(useApp.getState().project).toBe(before);
  });

  it('leaves the antenna alone when no trial finished', async () => {
    vi.spyOn(transport, 'optimize').mockResolvedValue({ ...outcome(), best: null, stop: { kind: 'cancelled' } });
    const before = useApp.getState().project;
    await useApp.getState().runOptimization();
    expect(useApp.getState().project).toBe(before);
    expect(useApp.getState().optimizer.outcome?.stop.kind).toBe('cancelled');
  });

  it('drops a result for a document that was closed', async () => {
    let finish!: (o: OptimizeOutcome) => void;
    vi.spyOn(transport, 'optimize').mockReturnValue(new Promise((r) => (finish = r)));
    const run = useApp.getState().runOptimization();
    useApp.getState().load({ project: newProject(), warnings: [] }, 'other.maa', null);
    finish(outcome());
    await run;
    expect(useApp.getState().optimizer.outcome).toBeNull();
    expect(useApp.getState().project.antenna.wires[0].end.y).not.toBe(5.5);
  });

  it('writes a trial back', async () => {
    vi.spyOn(transport, 'calculate').mockResolvedValue({ warnings: [] } as unknown as CalcResult);
    vi.spyOn(transport, 'optimize').mockResolvedValue(outcome());
    await useApp.getState().runOptimization();
    const applied = structuredClone(useApp.getState().project);
    applied.antenna.wires[0].end.y = 4.9;
    const spy = vi.spyOn(transport, 'applyTrial').mockResolvedValue({ project: applied, frequencyMhz: 14.2 });
    await useApp.getState().applyTrial([4.9]);
    expect(spy.mock.calls[0][0].values).toEqual([4.9]);
    expect(useApp.getState().project.antenna.wires[0].end.y).toBe(4.9);
  });
});
