// The wire tools' dialogs (アンテナのサイズ, 検索と置換): open or not, and the
// wire-table column 検索と置換 starts on.
import { create } from 'zustand';

/** A wire-table column, as its heading reads. */
export type WireColumn = 'X1' | 'Y1' | 'Z1' | 'X2' | 'Y2' | 'Z2' | 'R' | 'SEG';

export const WIRE_COLUMNS: readonly WireColumn[] = ['X1', 'Y1', 'Z1', 'X2', 'Y2', 'Z2', 'R', 'SEG'];

interface WireToolsState {
  scaleOpen: boolean;
  /** The column 検索と置換 was opened on, or null when it is closed. */
  replaceColumn: WireColumn | null;
  setScaleOpen: (open: boolean) => void;
  openReplace: (column: WireColumn) => void;
  closeReplace: () => void;
}

export const useWireTools = create<WireToolsState>()((set) => ({
  scaleOpen: false,
  replaceColumn: null,
  setScaleOpen: (scaleOpen) => set({ scaleOpen }),
  openReplace: (replaceColumn) => set({ replaceColumn }),
  closeReplace: () => set({ replaceColumn: null }),
}));
