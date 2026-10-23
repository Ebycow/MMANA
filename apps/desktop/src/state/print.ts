// What to print, remembered per viewer like the display settings, and
// whether the print preview is open.
import { create } from 'zustand';
import type { PolarScale } from '../charts/polar';
import { SECTIONS, type SectionId } from '../domain/print';
import { useDisplay } from './display';

export type PrintUnit = 'm' | 'mm' | 'λ';

interface Saved {
  sections: Record<SectionId, boolean>;
  /** Null follows the display settings. */
  unit: PrintUnit | null;
  polarScale: PolarScale | null;
}

export interface PrintState extends Saved {
  open: boolean;
  setOpen: (open: boolean) => void;
  setSection: (id: SectionId, on: boolean) => void;
  setUnit: (unit: PrintUnit) => void;
  setPolarScale: (scale: PolarScale) => void;
}

const KEY = 'mmana.print';

const initial = (): Saved => ({
  sections: Object.fromEntries(SECTIONS.map((s) => [s.id, s.initial])) as Record<SectionId, boolean>,
  unit: null,
  polarScale: null,
});

function load(): Saved {
  const defaults = initial();
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return defaults;
    const saved = JSON.parse(raw) as Partial<Saved>;
    // sections added since keep their default
    return { ...defaults, ...saved, sections: { ...defaults.sections, ...saved.sections } };
  } catch {
    return defaults;
  }
}

export const usePrint = create<PrintState>()((set, get) => {
  const save = (patch: Partial<Saved>) => {
    set(patch);
    const { sections, unit, polarScale } = get();
    try {
      localStorage.setItem(KEY, JSON.stringify({ sections, unit, polarScale }));
    } catch {
      // not remembered; the defaults apply next time
    }
  };
  return {
    ...load(),
    open: false,
    setOpen: (open) => set({ open }),
    setSection: (id, on) => save({ sections: { ...get().sections, [id]: on } }),
    setUnit: (unit) => save({ unit }),
    setPolarScale: (polarScale) => save({ polarScale }),
  };
});

/** The unit lengths print in: the one chosen, else the one the tables show. */
export function usePrintUnit(): PrintUnit {
  const unit = usePrint((s) => s.unit);
  const display = useDisplay();
  return unit ?? (display.wavelengths ? 'λ' : display.millimetres ? 'mm' : 'm');
}

export function usePrintPolarScale(): PolarScale {
  const scale = usePrint((s) => s.polarScale);
  const display = useDisplay((s) => s.polarScale);
  return scale ?? display;
}
