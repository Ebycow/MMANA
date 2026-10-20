import type { Antenna, CalcOptions, Environment, Load, Project, Source, Wire } from '../api/types';

export const SPEED_OF_LIGHT = 299.8;

export const wavelength = (mhz: number) => (mhz > 0 ? SPEED_OF_LIGHT / mhz : 0);

export function defaultEnvironment(): Environment {
  return {
    ground: 'real',
    height: 20,
    wireMaterial: 'lossless',
    swrReference: { r: 50, x: 0 },
    fbRange: { horizontal: 120, vertical: 60 },
    realGround: { media: [{ permittivity: 5, conductivity: 0.001, boundary: null, height: null }], screen: null },
  };
}

/** A half-wave dipole for 14.05 MHz, MMANA's default band. */
export function newProject(): Project {
  const f = 14.05;
  const half = Math.round((wavelength(f) * 0.48) / 2 * 1000) / 1000;
  const antenna: Antenna = {
    name: '新規アンテナ',
    frequencyMhz: f,
    wires: [newWire({ x: 0, y: -half, z: 0 }, { x: 0, y: half, z: 0 })],
    sources: [{ pulse: 'w1c', phaseDeg: 0, voltage: 1 }],
    autoVoltage: true,
    loads: [],
    loadsEnabled: true,
    autoSegmentation: { dm1: 400, dm2: 40, sc: 2, ec: 1 },
    stack: { horizontal: 1, vertical: 1, horizontalSpacing: wavelength(f), verticalSpacing: wavelength(f), direction: 'centered' },
    combinedWires: [],
    comment: '',
  };
  return { antenna, environment: defaultEnvironment() };
}

export function newWire(start: Wire['start'], end: Wire['end']): Wire {
  return { start, end, radius: 0.001, segmentation: { kind: 'taper' }, note: '' };
}

export const newSource = (): Source => ({ pulse: 'w1c', phaseDeg: 0, voltage: 1 });

/**
 * MMANA's `SetAutoVol`: with automatic voltage on, every source gets
 * 1 / count. MMANA runs it whenever a source is added or removed, not when
 * the setting itself is switched on.
 */
export function setAutoVoltage(antenna: Antenna) {
  if (!antenna.autoVoltage) return;
  for (const s of antenna.sources) s.voltage = 1 / antenna.sources.length;
}
export const newLoad = (): Load => ({ pulse: 'w1c', kind: 'impedance', r: 0, x: 0 });

export function defaultOptions(): CalcOptions {
  return {
    frequencyMhz: null,
    patternResolution: 'fine',
    fixFreeSpaceElevation: true,
    horizontalCutElevationDeg: null,
    maxPulses: 4096,
  };
}

export function wireLength(w: Wire): number {
  const dx = w.end.x - w.start.x;
  const dy = w.end.y - w.start.y;
  const dz = w.end.z - w.start.z;
  return Math.hypot(dx, dy, dz);
}

/** Resolves the wire a pulse name such as `w3c` / `w3b2` refers to (1-based), or null. */
export function wireOfPulse(name: string): number | null {
  const m = /^w(\d+)/i.exec(name.trim());
  return m ? Number(m[1]) : null;
}
