// Which problems the viewer has hidden, remembered per viewer like the
// display settings, and the problems to show for the antenna as it is now.
import { useMemo } from 'react';
import { create } from 'zustand';
import { RULES, collectDiagnostics, shown, type Diagnostic, type RuleId } from '../domain/diagnostics';
import { useApp } from './store';

export interface ChecksState {
  /** Rules whose problems are not marked; errors show whatever this says. */
  hidden: RuleId[];
  dialogOpen: boolean;
  setHidden: (id: RuleId, hide: boolean) => void;
  setDialogOpen: (open: boolean) => void;
}

const KEY = 'mmana.checks';

function load(): RuleId[] {
  try {
    const raw = localStorage.getItem(KEY);
    const saved = raw ? (JSON.parse(raw) as { hidden?: unknown }).hidden : [];
    // rules removed since are dropped
    return Array.isArray(saved) ? saved.filter((id): id is RuleId => RULES.some((r) => r.id === id)) : [];
  } catch {
    return [];
  }
}

export const useChecks = create<ChecksState>()((set, get) => ({
  hidden: load(),
  dialogOpen: false,
  setHidden: (id, hide) => {
    const hidden = get().hidden.filter((h) => h !== id);
    if (hide) hidden.push(id);
    set({ hidden });
    try {
      localStorage.setItem(KEY, JSON.stringify({ hidden }));
    } catch {
      // not remembered; every rule shows next time
    }
  },
  setDialogOpen: (dialogOpen) => set({ dialogOpen }),
}));

/**
 * The problems to mark, errors first. A calculation's error and warnings
 * count only while they are of the antenna as it is: after an edit they go
 * until the next calculation, so a fixed wire does not stay marked.
 */
export function useDiagnostics(): Diagnostic[] {
  const project = useApp((s) => s.project);
  const calcError = useApp((s) => s.calcError);
  const errorProject = useApp((s) => s.calcErrorProject);
  const result = useApp((s) => s.result);
  const resultStale = useApp((s) => s.resultStale);
  const hidden = useChecks((s) => s.hidden);
  const wires = project.antenna.wires;
  const error = calcError && errorProject === project ? calcError : null;
  const current = result && !resultStale ? result : null;
  return useMemo(() => shown(collectDiagnostics({ wires, error, result: current }), hidden), [wires, error, current, hidden]);
}
