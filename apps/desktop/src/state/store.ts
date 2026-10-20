import { create } from 'zustand';
import type { CalcOptions, CalcResult, OptimizeOutcome, OptimizerSetup, ParsedProject, Progress, Project, SweepPoint } from '../api/types';
import { EngineError, files, transport } from '../api/transport';
import { defaultSetup } from '../domain/optimize';
import { defaultOptions, newProject } from '../domain/project';

const HISTORY_LIMIT = 200;

export interface SweepState {
  startMhz: number;
  stopMhz: number;
  points: number;
  data: SweepPoint[] | null;
  /** The project or options changed since `data` was computed. */
  stale: boolean;
  running: boolean;
}

/** Calculations kept for the optimisation log. */
const LOG_LIMIT = 300;

export interface OptimizerState {
  /** MMANA's optimisation sheet; kept until another document is opened. */
  setup: OptimizerSetup;
  dialogOpen: boolean;
  resultsOpen: boolean;
  running: boolean;
  /** Calculations of the run, newest last (the improving ones always, others now and then). */
  log: Progress[];
  /** The calculation with the best score so far. */
  best: Progress | null;
  startedAt: number | null;
  finishedAt: number | null;
  outcome: OptimizeOutcome | null;
  /** The setup the outcome's trials have values for. */
  outcomeSetup: OptimizerSetup | null;
  /**
   * The antenna and options the run started from. Trials are written back to
   * these: their element numbers are those of the starting antenna, which a
   * trial may change (moving an element past another renumbers them).
   */
  outcomeStart: { project: Project; options: CalcOptions } | null;
  /** The values of the trial the antenna was last set to (the best, or one written back). */
  appliedValues: number[] | null;
  error: EngineError | null;
}

const freshOptimizer = (): OptimizerState => ({
  setup: defaultSetup(),
  dialogOpen: false,
  resultsOpen: false,
  running: false,
  log: [],
  best: null,
  startedAt: null,
  finishedAt: null,
  outcome: null,
  outcomeSetup: null,
  outcomeStart: null,
  appliedValues: null,
  error: null,
});

export interface AppState {
  project: Project;
  fileName: string;
  filePath: string | null;
  dirty: boolean;
  past: Project[];
  future: Project[];

  /** 0-based index of the selected wire. */
  selectedWire: number | null;

  options: CalcOptions;
  autoCalc: boolean;
  calculating: boolean;
  calcQueued: boolean;
  result: CalcResult | null;
  /** The project changed since `result` was computed. */
  resultStale: boolean;
  calcError: EngineError | null;
  lastCalcMs: number | null;

  sweep: SweepState;
  showCurrents: boolean;
  /** The element editor (エレメント編集) is open; it edits a copy of the antenna. */
  elementEditorOpen: boolean;
  optimizer: OptimizerState;

  /** Applies an edit to a copy of the project and records it for undo. */
  edit: (recipe: (draft: Project) => void) => void;
  undo: () => void;
  redo: () => void;
  load: (parsed: ParsedProject, name: string, path: string | null) => void;
  reset: () => void;
  selectWire: (index: number | null) => void;
  setOptions: (patch: Partial<CalcOptions>) => void;
  setAutoCalc: (on: boolean) => void;
  setShowCurrents: (on: boolean) => void;
  setElementEditorOpen: (open: boolean) => void;
  setSweepRange: (patch: Partial<Pick<SweepState, 'startMhz' | 'stopMhz' | 'points'>>) => void;
  calculate: () => Promise<void>;
  runSweep: () => Promise<void>;
  save: (saveAs: boolean) => Promise<boolean>;
  setOptimizer: (patch: Partial<OptimizerState>) => void;
  /** Optimises the antenna with the sheet and applies the result as one undoable edit. */
  runOptimization: () => Promise<void>;
  cancelOptimization: () => Promise<void>;
  /** Writes the values of a trial of the last run back (the trial list's 更新). */
  applyTrial: (values: number[]) => Promise<void>;
}

const sweepAround = (mhz: number): Pick<SweepState, 'startMhz' | 'stopMhz'> => ({
  startMhz: Math.round(mhz * 0.98 * 1000) / 1000,
  stopMhz: Math.round(mhz * 1.02 * 1000) / 1000,
});

const initialProject = newProject();

const freshDocument = (project: Project) => ({
  project,
  past: [] as Project[],
  future: [] as Project[],
  dirty: false,
  selectedWire: null,
  result: null,
  resultStale: false,
  calcError: null,
  sweep: { ...sweepAround(project.antenna.frequencyMhz), points: 21, data: null, stale: false, running: false },
  optimizer: freshOptimizer(),
});

const toEngineError = (e: unknown) =>
  e instanceof EngineError ? e : new EngineError('internal', 'Unexpected error', String(e));

export const useApp = create<AppState>()((set, get) => {
  // bumped when another document is opened, so results still in flight for
  // the previous one are dropped
  let generation = 0;
  // the latest sweep request; an earlier one finishing must not overwrite it
  let sweepRequest = 0;

  /** Marks the result and sweep of the current document as out of date. */
  const staleFlags = () => ({
    resultStale: get().result != null,
    sweep: { ...get().sweep, stale: get().sweep.data != null },
  });

  /** The calculation frequency an optimisation left, unless it is today's. */
  const keepFrequency = (mhz: number) => {
    const { options, project } = get();
    if (mhz !== (options.frequencyMhz ?? project.antenna.frequencyMhz)) get().setOptions({ frequencyMhz: mhz });
  };
  const applyProject = (p: Project, mhz: number) => {
    get().edit((draft) => {
      draft.antenna = structuredClone(p.antenna);
      draft.environment = structuredClone(p.environment);
    });
    keepFrequency(mhz);
  };

  const openDocument = (project: Project, fileName: string, filePath: string | null) => {
    generation++;
    sweepRequest++;
    set({ ...freshDocument(project), fileName, filePath, calcQueued: false });
  };

  return {
    ...freshDocument(initialProject),
    fileName: '無題.maa',
    filePath: null,
    options: defaultOptions(),
    autoCalc: false,
    calculating: false,
    calcQueued: false,
    lastCalcMs: null,
    showCurrents: true,
    elementEditorOpen: false,

    edit: (recipe) => {
      const { project, past } = get();
      const draft = structuredClone(project);
      recipe(draft);
      set({
        project: draft,
        past: [...past, project].slice(-HISTORY_LIMIT),
        future: [],
        dirty: true,
        ...staleFlags(),
      });
    },
    undo: () => {
      const { past, project, future } = get();
      const prev = past.at(-1);
      if (!prev) return;
      set({ project: prev, past: past.slice(0, -1), future: [project, ...future], dirty: true, ...staleFlags() });
    },
    redo: () => {
      const { past, project, future } = get();
      const next = future[0];
      if (!next) return;
      set({ project: next, past: [...past, project], future: future.slice(1), dirty: true, ...staleFlags() });
    },
    load: (parsed, name, path) => openDocument(parsed.project, name, path),
    reset: () => openDocument(newProject(), '無題.maa', null),
    selectWire: (index) => set({ selectedWire: index }),
    setOptions: (patch) => set({ options: { ...get().options, ...patch }, ...staleFlags() }),
    setAutoCalc: (on) => set({ autoCalc: on }),
    setShowCurrents: (on) => set({ showCurrents: on }),
    setElementEditorOpen: (open) => set({ elementEditorOpen: open }),
    setSweepRange: (patch) => set({ sweep: { ...get().sweep, ...patch } }),

    calculate: async () => {
      // a request during a running calculation runs once it finishes
      if (get().calculating) {
        set({ calcQueued: true });
        return;
      }
      const project = get().project;
      const options = get().options;
      const gen = generation;
      set({ calculating: true, calcQueued: false });
      const t0 = performance.now();
      try {
        const result = await transport.calculate({ project, options });
        if (gen === generation) {
          // edits made while calculating keep the result marked stale
          const stale = get().project !== project || get().options !== options;
          set({ result, calcError: null, resultStale: stale, lastCalcMs: performance.now() - t0 });
        }
      } catch (e) {
        if (gen === generation) set({ calcError: toEngineError(e) });
      } finally {
        set({ calculating: false });
      }
      if (get().calcQueued) await get().calculate();
    },

    runSweep: async () => {
      const { project, options, sweep } = get();
      const request = ++sweepRequest;
      set({ sweep: { ...sweep, running: true } });
      try {
        const res = await transport.sweep({
          project,
          options: { ...options, patternResolution: 'coarse' },
          startMhz: sweep.startMhz,
          stopMhz: sweep.stopMhz,
          points: sweep.points,
        });
        if (request !== sweepRequest) return;
        const stale = get().project !== project || get().options !== options;
        set({ sweep: { ...get().sweep, data: res.points, stale, running: false }, calcError: null });
      } catch (e) {
        if (request !== sweepRequest) return;
        set({ sweep: { ...get().sweep, running: false }, calcError: toEngineError(e) });
      }
    },

    save: async (saveAs) => {
      const { project, filePath, fileName } = get();
      const gen = generation;
      const saved = await files.save(project, saveAs ? null : filePath, fileName);
      if (!saved) return false;
      // a document opened while saving keeps its own name and state
      if (gen !== generation) return true;
      if (saved.path) set({ filePath: saved.path, fileName: saved.path.split(/[\\/]/).pop() ?? fileName });
      // the file holds `project`; edits made while saving are still unsaved
      set({ dirty: get().project !== project });
      return true;
    },

    setOptimizer: (patch) => set({ optimizer: { ...get().optimizer, ...patch } }),

    runOptimization: async () => {
      const { project, options, optimizer } = get();
      if (optimizer.running) return;
      const setup = optimizer.setup;
      const gen = generation;
      const patch = (p: Partial<OptimizerState>) => gen === generation && get().setOptimizer(p);
      patch({ running: true, log: [], best: null, error: null, startedAt: performance.now(), finishedAt: null });
      try {
        const outcome = await transport.optimize({ project, options, setup, maxCalculations: null }, (p) => {
          const o = get().optimizer;
          patch({ log: [...o.log.slice(1 - LOG_LIMIT), p], best: p.improved ? p : o.best });
        });
        if (gen !== generation) return;
        if (outcome.best) applyProject(outcome.project, outcome.frequencyMhz);
        patch({
          running: false,
          finishedAt: performance.now(),
          outcome,
          outcomeSetup: setup,
          outcomeStart: { project, options },
          appliedValues: outcome.best?.values ?? null,
        });
        if (outcome.best) await get().calculate();
      } catch (e) {
        patch({ running: false, finishedAt: performance.now(), error: toEngineError(e) });
      }
    },

    cancelOptimization: () => transport.cancelOptimize(),

    applyTrial: async (values) => {
      const { outcomeSetup: setup, outcomeStart: start } = get().optimizer;
      if (!setup || !start) return;
      const gen = generation;
      const applied = await transport.applyTrial({ ...start, setup, values });
      if (gen !== generation) return;
      applyProject(applied.project, applied.frequencyMhz);
      get().setOptimizer({ appliedValues: values });
      await get().calculate();
    },
  };
});
