// Where the wire list's 追加 button puts a new wire. Remembered per viewer like
// the display settings; not part of the document.
import { create } from 'zustand';
import type { NewWireSettings } from '../domain/wireEdit';

interface NewWireState extends NewWireSettings {
  set: (patch: Partial<NewWireSettings>) => void;
}

const KEY = 'mmana.newWire';
// MMANA's new row: every coordinate 0, typed in afterwards
const DEFAULTS: NewWireSettings = {
  mode: 'zero',
  offset: { x: 0.1, y: 0, z: 0 },
  start: { x: 0, y: 0, z: 0 },
  end: { x: 0, y: 1, z: 0 },
};

function load(): NewWireSettings {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<NewWireSettings>) } : DEFAULTS;
  } catch {
    return DEFAULTS;
  }
}

export const useNewWire = create<NewWireState>()((set, get) => ({
  ...load(),
  set: (patch) => {
    set(patch);
    const { mode, offset, start, end } = get();
    try {
      localStorage.setItem(KEY, JSON.stringify({ mode, offset, start, end }));
    } catch {
      // not remembered; the defaults apply next time
    }
  },
}));
