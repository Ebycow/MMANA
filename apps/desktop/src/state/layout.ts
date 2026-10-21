// Which of the window's three panels are shown. The workspace collapses a
// hidden panel instead of unmounting it, so the editor's tab and the 3D
// camera survive hiding; the resizable groups remember the sizes, and this
// mirrors which panels are open so the header can toggle them.
import { create } from 'zustand';

export type PaneId = 'editor' | 'view' | 'results';
export type Shown = Record<PaneId, boolean>;

export interface LayoutState {
  shown: Shown;
  /** Shows or hides a panel; the last panel shown stays. */
  setShown: (pane: PaneId, on: boolean) => void;
  /** Takes over the panels the workspace has open (restored, or dragged shut or open). */
  sync: (shown: Shown) => void;
}

const same = (a: Shown, b: Shown) => a.editor === b.editor && a.view === b.view && a.results === b.results;

export const useLayout = create<LayoutState>()((set, get) => ({
  shown: { editor: true, view: true, results: true },
  setShown: (pane, on) => {
    const shown = { ...get().shown, [pane]: on };
    if (shown.editor || shown.view || shown.results) set({ shown });
  },
  sync: (shown) => !same(shown, get().shown) && set({ shown }),
}));
