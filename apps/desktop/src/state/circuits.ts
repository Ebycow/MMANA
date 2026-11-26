// The calculator dialog (ツール → 計算ツール…): open or not, and its page.
import { create } from 'zustand';

export type CircuitTab = 'tank' | 'coil' | 'match' | 'stub' | 'line' | 'twoSection' | 'stubMatch';

interface CircuitsState {
  open: boolean;
  tab: CircuitTab;
  setOpen: (open: boolean) => void;
  setTab: (tab: CircuitTab) => void;
}

export const useCircuits = create<CircuitsState>()((set) => ({
  open: false,
  tab: 'tank',
  setOpen: (open) => set({ open }),
  setTab: (tab) => set({ tab }),
}));
