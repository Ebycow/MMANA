import { describe, expect, it } from 'vitest';
import { CABLES, DEFAULT_STUB_CABLE, sig, stubGiven, tankInput, type StubValues, type TankValues } from '../domain/circuits';

const tank = (patch: Partial<TankValues> = {}): TankValues => ({
  frequencyMhz: 14.05,
  reactanceOhm: 50,
  inductanceUh: 1,
  capacitancePf: 100,
  fixedFrequency: true,
  last: 'reactance',
  ...patch,
});

describe('LC resonance edits', () => {
  it('keeps the frequency when it is fixed', () => {
    expect(tankInput(tank(), 'reactance')).toEqual({ from: 'reactance', frequencyMhz: 14.05, reactanceOhm: 50 });
    expect(tankInput(tank(), 'inductance')).toEqual({ from: 'inductance', frequencyMhz: 14.05, inductanceUh: 1 });
    expect(tankInput(tank(), 'capacitance')).toEqual({ from: 'capacitance', frequencyMhz: 14.05, capacitancePf: 100 });
  });

  it('finds the frequency from L and C when it is not fixed', () => {
    expect(tankInput(tank({ fixedFrequency: false }), 'inductance')).toEqual({ from: 'resonance', inductanceUh: 1, capacitancePf: 100 });
    expect(tankInput(tank({ fixedFrequency: false }), 'capacitance')).toEqual({ from: 'resonance', inductanceUh: 1, capacitancePf: 100 });
    // X still keeps it
    expect(tankInput(tank({ fixedFrequency: false }), 'reactance')?.from).toBe('reactance');
    // without the other, the frequency stays
    expect(tankInput(tank({ fixedFrequency: false, capacitancePf: null }), 'inductance')?.from).toBe('inductance');
  });

  it('keeps the value typed last when the frequency changes', () => {
    expect(tankInput(tank({ last: 'inductance' }), 'frequency')).toEqual({ from: 'inductance', frequencyMhz: 14.05, inductanceUh: 1 });
    expect(tankInput(tank({ last: 'capacitance', fixedFrequency: false }), 'frequency')?.from).toBe('capacitance');
    expect(tankInput(tank({ last: 'reactance' }), 'frequency')?.from).toBe('reactance');
  });

  it('asks for nothing while a value it needs is blank', () => {
    expect(tankInput(tank({ reactanceOhm: null }), 'reactance')).toBeNull();
    expect(tankInput(tank({ frequencyMhz: null }), 'inductance')).toBeNull();
  });
});

describe('stub edits', () => {
  const v: StubValues = { reactanceOhm: 25, value: 1.5, inductive: true, lengthCm: 80 };

  it('calculates from the field edited', () => {
    expect(stubGiven(v, 'reactance')).toEqual({ from: 'reactance', reactanceOhm: 25 });
    expect(stubGiven(v, 'length')).toEqual({ from: 'length', lengthCm: 80 });
    expect(stubGiven(v, 'value')).toEqual({ from: 'inductance', inductanceUh: 1.5 });
    expect(stubGiven({ ...v, inductive: false }, 'value')).toEqual({ from: 'capacitance', capacitancePf: 1.5 });
  });

  it('keeps L or C for a new frequency or cable, and the length for the other end', () => {
    expect(stubGiven(v, 'other').from).toBe('inductance');
    expect(stubGiven(v, 'end').from).toBe('length');
  });
});

describe('cables', () => {
  it('rounds the velocity factors to floats as the original does', () => {
    const c = CABLES.find((c) => c.name === DEFAULT_STUB_CABLE)!;
    expect(c.impedanceOhm).toBe(50);
    expect(c.velocityFactor).toBe(Math.fround(0.67));
    expect(c.velocityFactor).not.toBe(0.67);
  });

  it('has the original 19 cables, each named once', () => {
    expect(CABLES).toHaveLength(19);
    expect(new Set(CABLES.map((c) => c.name)).size).toBe(19);
  });
});

describe('values', () => {
  it('shows 6 significant digits and blanks', () => {
    expect(sig(0.56638769783)).toBe('0.566388');
    expect(sig(226.555079134)).toBe('226.555');
    expect(sig(null)).toBe('');
    expect(sig(Number.NaN)).toBe('');
  });
});
