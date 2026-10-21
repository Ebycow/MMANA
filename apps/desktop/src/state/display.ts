// How lengths are shown, shared by the wire table and the element editor like
// MMANA's 波長表記 check box and 通常単位をmmで表示 menu item. Remembered per
// viewer, like MMANA's ini file; not part of the document.
import { create } from 'zustand';

export interface DisplayState {
  /** Coordinates and sizes in mm instead of m (the radius is always in mm). */
  millimetres: boolean;
  /** Lengths and the radius in wavelengths at the design frequency; takes precedence over `millimetres`. */
  wavelengths: boolean;
  setMillimetres: (on: boolean) => void;
  setWavelengths: (on: boolean) => void;
}

const KEY = 'mmana.display';
// MMANA starts with lengths in mm (exeenv.MmSel = 1)
const DEFAULTS = { millimetres: true, wavelengths: false };

function load(): typeof DEFAULTS {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<typeof DEFAULTS>) } : DEFAULTS;
  } catch {
    return DEFAULTS;
  }
}

export const useDisplay = create<DisplayState>()((set, get) => {
  const save = (patch: Partial<typeof DEFAULTS>) => {
    set(patch);
    const { millimetres, wavelengths } = get();
    try {
      localStorage.setItem(KEY, JSON.stringify({ millimetres, wavelengths }));
    } catch {
      // not remembered; the defaults apply next time
    }
  };
  return {
    ...load(),
    setMillimetres: (millimetres) => save({ millimetres }),
    setWavelengths: (wavelengths) => save({ wavelengths }),
  };
});
