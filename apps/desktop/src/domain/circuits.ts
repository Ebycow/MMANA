// The calculators of MMANA's 共振条件 dialog: which calculation an edit of a
// field asks for (the dialog's edit handlers), the cables of its stub page,
// and how the values are shown. The calculations are in mmana-core
// (crates/mmana-core/src/circuits.rs).
import type { CircuitInput } from '../api/types';

/** A cable of the stub page (`CoreTypeT`, `CorePT`, `CoreZT` in OptDlg.cpp). */
export interface CablePreset {
  name: string;
  /** Rounded to a float like the original's table. */
  velocityFactor: number;
  impedanceOhm: number;
}

const f = Math.fround;

export const CABLES: CablePreset[] = [
  { name: 'はしごフィーダー 600 Ω', velocityFactor: f(0.98), impedanceOhm: 600 },
  { name: 'はしごフィーダー 450 Ω', velocityFactor: f(0.98), impedanceOhm: 450 },
  { name: 'リボンフィーダー 300 Ω', velocityFactor: f(0.85), impedanceOhm: 300 },
  { name: '3C-2V', velocityFactor: f(0.67), impedanceOhm: 75 },
  { name: '5C-2V', velocityFactor: f(0.67), impedanceOhm: 75 },
  { name: '7C-2V', velocityFactor: f(0.67), impedanceOhm: 75 },
  { name: '10C-2V', velocityFactor: f(0.67), impedanceOhm: 75 },
  { name: '5D-2V', velocityFactor: f(0.67), impedanceOhm: 50 },
  { name: '8D-2V', velocityFactor: f(0.67), impedanceOhm: 50 },
  { name: 'RG-58/U', velocityFactor: f(0.67), impedanceOhm: 53.5 },
  { name: 'RG-58A/U', velocityFactor: f(0.67), impedanceOhm: 50 },
  { name: '5D-FB', velocityFactor: f(0.81), impedanceOhm: 50 },
  { name: '8D-FB', velocityFactor: f(0.81), impedanceOhm: 50 },
  { name: '10D-FB', velocityFactor: f(0.8), impedanceOhm: 50 },
  { name: '12D-FB', velocityFactor: f(0.81), impedanceOhm: 50 },
  { name: '5D-SFA', velocityFactor: f(0.83), impedanceOhm: 50 },
  { name: '8D-SFA', velocityFactor: f(0.83), impedanceOhm: 50 },
  { name: '10D-SFA', velocityFactor: f(0.88), impedanceOhm: 50 },
  { name: '12D-SFA', velocityFactor: f(0.88), impedanceOhm: 50 },
];

/** The original's default cable for a stub (5D-2V). */
export const DEFAULT_STUB_CABLE = '5D-2V';

/** 6 significant digits, without trailing zeros; '' for a blank value. */
export function sig(v: number | null | undefined, digits = 6): string {
  if (v == null || !Number.isFinite(v)) return '';
  return String(Number(v.toPrecision(digits)));
}

// ---------------------------------------------------------- LC resonance ---

export type TankField = 'frequency' | 'reactance' | 'inductance' | 'capacitance';

export interface TankValues {
  frequencyMhz: number | null;
  reactanceOhm: number | null;
  inductanceUh: number | null;
  capacitancePf: number | null;
  /** 周波数を固定: an edit of L or C keeps the frequency (else it keeps the other of L and C). */
  fixedFrequency: boolean;
  /** Of X, L and C, the one typed last: a new frequency keeps it. */
  last: 'reactance' | 'inductance' | 'capacitance';
}

/** The calculation an edit of `field` asks for; null when a value it needs is blank. */
export function tankInput(v: TankValues, field: TankField): CircuitInput<'lcResonance'> | null {
  const f = v.frequencyMhz;
  const from = field === 'frequency' ? v.last : field;
  if (from === 'reactance') return f != null && v.reactanceOhm != null ? { from, frequencyMhz: f, reactanceOhm: v.reactanceOhm } : null;
  const l = v.inductanceUh;
  const c = v.capacitancePf;
  // without the frequency fixed, L and C give it (CalcTank(3))
  if (field !== 'frequency' && !v.fixedFrequency && l != null && c != null) return { from: 'resonance', inductanceUh: l, capacitancePf: c };
  if (f == null) return null;
  if (from === 'inductance') return l != null ? { from, frequencyMhz: f, inductanceUh: l } : null;
  return c != null ? { from, frequencyMhz: f, capacitancePf: c } : null;
}

// --------------------------------------------------------------- stub ---

export type StubField = 'reactance' | 'value' | 'length' | 'other' | 'end';

export interface StubValues {
  reactanceOhm: number | null;
  /** µH when the reactance is inductive (≥ 0), pF otherwise. */
  value: number;
  inductive: boolean;
  lengthCm: number;
}

/**
 * What the stub is calculated from after an edit of `field`. Like the
 * original, a new frequency or cable keeps L or C (CalcCore(0)), and the
 * other end keeps the length (CStubClick).
 */
export function stubGiven(v: StubValues, field: StubField): CircuitInput<'stub'>['given'] {
  switch (field) {
    case 'reactance':
      return { from: 'reactance', reactanceOhm: v.reactanceOhm ?? 0 };
    case 'length':
    case 'end':
      return { from: 'length', lengthCm: v.lengthCm };
    default:
      return v.inductive ? { from: 'inductance', inductanceUh: v.value } : { from: 'capacitance', capacitancePf: v.value };
  }
}
