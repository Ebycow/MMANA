import { create } from 'zustand';
import type { CalcOptions, CalcResult, OptimizeOutcome, OptimizerSetup, ParsedProject, Progress, Project, SweepPoint } from '../api/types';
import { EngineError, files, transport } from '../api/transport';
import { defaultSetup } from '../domain/optimize';
import { defaultOptions, newProject } from '../domain/project';
import { OUTLINE_POINTS, sweepOrder } from '../domain/sweep';

const HISTORY_LIMIT = 200;

export interface SweepState {
  startMhz: number;
  stopMhz: number;
  points: number;
  /** By frequency; while a sweep runs, the points calculated so far. */
  data: SweepPoint[] | null;
  /** The project or options changed since `data` was computed. */
  stale: boolean;
  running: boolean;
  /** Points of the running sweep calculated so far; kept when it was stopped short. */
  progress: { done: number; total: number } | null;
  /** 中止 was pressed: the sweep ends when the points being calculated arrive. */
  stopping: boolean;
}

/** A sweep is sent in batches that take about this long, so its points appear as they are done. */
const SWEEP_BATCH_MS = 1000;
/** Smallest batch after the outline (keeps the cores busy on a large model). */
const SWEEP_MIN_BATCH = 4;

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

/**
 * A step of the undo history: the project before it, and the calculation
 * options before it when the step changed them too (an optimisation that
 * moved the frequency). Other changes of the options are not undone.
 */
export interface HistoryEntry {
  project: Project;
  options?: CalcOptions;
}

export interface AppState {
  project: Project;
  fileName: string;
  filePath: string | null;
  dirty: boolean;
  past: HistoryEntry[];
  future: HistoryEntry[];

  /** 0-based indices of the selected wires, in the order they were picked. */
  selectedWires: number[];
  /** The wire last picked (the table's current row), or null. */
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

  /**
   * Applies an edit to a copy of the project and records it for undo; an
   * options patch is applied and undone with it.
   */
  edit: (recipe: (draft: Project) => void, options?: Partial<CalcOptions>) => void;
  undo: () => void;
  redo: () => void;
  load: (parsed: ParsedProject, name: string, path: string | null) => void;
  reset: () => void;
  /** Selects only `index` (or nothing). */
  selectWire: (index: number | null) => void;
  /** Replaces the selection; `active` defaults to the last wire of it. */
  setSelection: (indices: number[], active?: number | null) => void;
  /** Adds a wire to the selection or removes it. */
  toggleWire: (index: number) => void;
  setOptions: (patch: Partial<CalcOptions>) => void;
  setAutoCalc: (on: boolean) => void;
  setShowCurrents: (on: boolean) => void;
  setElementEditorOpen: (open: boolean) => void;
  setSweepRange: (patch: Partial<Pick<SweepState, 'startMhz' | 'stopMhz' | 'points'>>) => void;
  calculate: () => Promise<void>;
  runSweep: () => Promise<void>;
  stopSweep: () => void;
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
  past: [] as HistoryEntry[],
  future: [] as HistoryEntry[],
  dirty: false,
  selectedWires: [] as number[],
  selectedWire: null,
  result: null,
  resultStale: false,
  calcError: null,
  sweep: { ...sweepAround(project.antenna.frequencyMhz), points: 21, data: null, stale: false, running: false, progress: null, stopping: false },
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

  /** Drops selected wires that an edit, undo or redo removed. */
  const keepSelection = (project: Project) => {
    const n = project.antenna.wires.length;
    const { selectedWires, selectedWire } = get();
    if (selectedWires.every((i) => i < n) && (selectedWire ?? -1) < n) return {};
    return { selectedWires: selectedWires.filter((i) => i < n), selectedWire: selectedWire != null && selectedWire < n ? selectedWire : null };
  };

  /** Marks the result and sweep of the current document as out of date. */
  const staleFlags = () => ({
    resultStale: get().result != null,
    sweep: { ...get().sweep, stale: get().sweep.data != null },
  });

  /** An optimised antenna and the calculation frequency it left, as one undoable step. */
  const applyProject = (p: Project, mhz: number) => {
    const { options, project } = get();
    const moved = mhz !== (options.frequencyMhz ?? project.antenna.frequencyMhz);
    get().edit(
      (draft) => {
        draft.antenna = structuredClone(p.antenna);
        draft.environment = structuredClone(p.environment);
      },
      moved ? { frequencyMhz: mhz } : undefined,
    );
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

    edit: (recipe, patch) => {
      const { project, past, options } = get();
      const draft = structuredClone(project);
      recipe(draft);
      set({
        project: draft,
        ...(patch && { options: { ...options, ...patch } }),
        past: [...past, patch ? { project, options } : { project }].slice(-HISTORY_LIMIT),
        future: [],
        dirty: true,
        ...staleFlags(),
        ...keepSelection(draft),
      });
    },
    undo: () => {
      const { past, project, future, options } = get();
      const prev = past.at(-1);
      if (!prev) return;
      set({
        project: prev.project,
        ...(prev.options && { options: prev.options }),
        past: past.slice(0, -1),
        future: [prev.options ? { project, options } : { project }, ...future],
        dirty: true,
        ...staleFlags(),
        ...keepSelection(prev.project),
      });
    },
    redo: () => {
      const { past, project, future, options } = get();
      const next = future[0];
      if (!next) return;
      set({
        project: next.project,
        ...(next.options && { options: next.options }),
        past: [...past, next.options ? { project, options } : { project }],
        future: future.slice(1),
        dirty: true,
        ...staleFlags(),
        ...keepSelection(next.project),
      });
    },
    load: (parsed, name, path) => openDocument(parsed.project, name, path),
    reset: () => openDocument(newProject(), '無題.maa', null),
    selectWire: (index) => set({ selectedWires: index == null ? [] : [index], selectedWire: index }),
    setSelection: (indices, active) => {
      const unique = [...new Set(indices)];
      set({ selectedWires: unique, selectedWire: active !== undefined ? active : (unique.at(-1) ?? null) });
    },
    toggleWire: (index) => {
      const { selectedWires } = get();
      if (selectedWires.includes(index)) {
        const rest = selectedWires.filter((i) => i !== index);
        set({ selectedWires: rest, selectedWire: rest.at(-1) ?? null });
      } else {
        set({ selectedWires: [...selectedWires, index], selectedWire: index });
      }
    },
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
      const { startMhz, stopMhz, points } = sweep;
      const order = sweepOrder(points);
      set({ sweep: { ...sweep, running: true, stopping: false, progress: { done: 0, total: order.length } } });
      let data: SweepPoint[] = [];
      let done = 0;
      let size = Math.min(OUTLINE_POINTS, order.length);
      try {
        while (done < order.length) {
          const indices = order.slice(done, done + size);
          const t0 = performance.now();
          const res = await transport.sweep({
            project,
            options: { ...options, patternResolution: 'coarse' },
            startMhz,
            stopMhz,
            points,
            indices,
          });
          if (request !== sweepRequest) return;
          done += indices.length;
          data = [...data, ...res.points].sort((a, b) => a.frequencyMhz - b.frequencyMhz);
          const stale = get().project !== project || get().options !== options;
          const finished = done >= order.length;
          const ends = finished || get().sweep.stopping;
          set({
            sweep: {
              ...get().sweep,
              data,
              stale,
              running: !ends,
              stopping: false,
              progress: finished ? null : { done, total: order.length },
            },
            calcError: null,
          });
          if (ends) return;
          const msPerPoint = (performance.now() - t0) / indices.length;
          size = Math.max(SWEEP_MIN_BATCH, Math.floor(SWEEP_BATCH_MS / Math.max(msPerPoint, 1e-3)));
        }
      } catch (e) {
        if (request !== sweepRequest) return;
        set({ sweep: { ...get().sweep, running: false, stopping: false }, calcError: toEngineError(e) });
      }
    },

    stopSweep: () => {
      if (get().sweep.running) set({ sweep: { ...get().sweep, stopping: true } });
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
