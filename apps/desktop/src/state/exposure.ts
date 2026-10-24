// Settings of the radio-protection check (電波防護) in the 3D view: the
// powers, the height of the plane and its range. They belong to the station
// rather than the antenna, so they are remembered per viewer and not saved in
// the document.
import { create } from 'zustand';
import type { ExposureSettings } from '../domain/exposure';

export interface ExposureState extends ExposureSettings {
  /** The 3D view shows the check. */
  on: boolean;
  setOn: (on: boolean) => void;
  set: (patch: Partial<ExposureSettings>) => void;
}

const KEY = 'mmana.exposure';
const DEFAULTS: ExposureSettings = { averageW: 100, peakW: 100, heightM: 1.5, rangeM: null };

function load(): ExposureSettings {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<ExposureSettings>) } : DEFAULTS;
  } catch {
    return DEFAULTS;
  }
}

export const useExposure = create<ExposureState>()((set, get) => ({
  ...load(),
  on: false,
  setOn: (on) => set({ on }),
  set: (patch) => {
    set(patch);
    const { averageW, peakW, heightM, rangeM } = get();
    try {
      localStorage.setItem(KEY, JSON.stringify({ averageW, peakW, heightM, rangeM }));
    } catch {
      // not remembered; the defaults apply next time
    }
  },
}));
