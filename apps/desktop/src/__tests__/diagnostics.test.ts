import { describe, expect, it } from 'vitest';
import { EngineError } from '../api/transport';
import type { CalcResult, Point3, Wire } from '../api/types';
import { RULES, byWire, canHide, collectDiagnostics, errorDiagnostic, shown, worst } from '../domain/diagnostics';
import { newWire } from '../domain/project';

const p = (x: number, y: number, z: number): Point3 => ({ x, y, z });
const w = (a: Point3, b: Point3): Wire => newWire(a, b);

// a square loop and a second loop sharing its bottom side (wire 5 on wire 1)
const loops = () => [
  w(p(0, 0, 0), p(1, 0, 0)),
  w(p(1, 0, 0), p(1, 0, 1)),
  w(p(1, 0, 1), p(0, 0, 1)),
  w(p(0, 0, 1), p(0, 0, 0)),
  w(p(0, 0, 0), p(1, 0, 0)),
];

const overlapError = () => new EngineError('calculation-failed', 'The antenna cannot be calculated', 'wires 1 and 5 lie exactly on top of each other', 'overlapping-wires', [1, 5]);

const result = (warnings: CalcResult['warnings']) => ({ warnings }) as unknown as CalcResult;

describe('gathering the problems of an antenna', () => {
  it('names the wires of a calculation error, 0-based', () => {
    expect(errorDiagnostic(overlapError())).toEqual({ rule: 'overlap', severity: 'error', wires: [0, 4], message: 'ワイヤー 1 と 5 が重なっています' });
  });

  it('ignores a calculation error that names no wires', () => {
    expect(errorDiagnostic(new EngineError('calculation-failed', 'x', 'the antenna has no sources', 'no-sources'))).toBeNull();
  });

  it('keeps an error of a kind it does not know, with the engine message', () => {
    const d = errorDiagnostic(new EngineError('calculation-failed', 'x', 'something new', 'something-new', [3]));
    expect(d).toMatchObject({ rule: 'engine-error', severity: 'error', wires: [2] });
    expect(d?.message).toContain('something new');
  });

  it('lists an overlap the engine and the connection check both found once', () => {
    const ds = collectDiagnostics({ wires: loops(), error: overlapError(), result: null });
    expect(ds.filter((d) => d.rule === 'overlap')).toHaveLength(1);
  });

  it('finds every overlap without calculating, though the engine stops at the first', () => {
    const ws = [...loops(), w(p(1, 0, 1), p(0, 0, 1))];
    const ds = collectDiagnostics({ wires: ws, error: overlapError(), result: null });
    expect(ds.filter((d) => d.rule === 'overlap').map((d) => d.wires)).toEqual([
      [0, 4],
      [2, 5],
    ]);
  });

  it('marks the wires a result warns about, but not warnings about the whole antenna', () => {
    const r = result([
      { code: 'tooManySegments', wires: [1, 2], message: '...' },
      { code: 'nonPositivePower', wires: [], message: '...' },
    ]);
    const ds = collectDiagnostics({ wires: loops().slice(0, 4), error: null, result: r });
    expect(ds).toEqual([expect.objectContaining({ rule: 'too-many-segments', severity: 'warning', wires: [0, 1] })]);
  });

  it('puts errors first, then warnings, then notes', () => {
    // wire 6 crosses the top side (a note from the connection check, found before the result's warning)
    const ws = [...loops(), w(p(0.5, 0, 0.5), p(0.5, 0, 2))];
    const ds = collectDiagnostics({ wires: ws, error: null, result: result([{ code: 'belowGround', wires: [6], message: '' }]) });
    expect(ds.map((d) => d.rule)).toEqual(['overlap', 'below-ground', 'cross']);
  });
});

describe('showing the problems', () => {
  it('hides the rules the viewer turned off, but never an error', () => {
    const ds = collectDiagnostics({ wires: loops(), error: null, result: result([{ code: 'belowGround', wires: [2], message: '' }]) });
    expect(shown(ds, ['below-ground', 'overlap']).map((d) => d.rule)).toEqual(['overlap']);
  });

  it('lets only warnings and notes be hidden', () => {
    expect(RULES.filter((r) => canHide(r.id)).every((r) => r.severity !== 'error')).toBe(true);
    expect(canHide('overlap')).toBe(false);
    expect(canHide('cross')).toBe(true);
  });

  it('groups the problems by wire and picks the worst', () => {
    const ds = collectDiagnostics({ wires: loops(), error: null, result: result([{ code: 'belowGround', wires: [1, 2], message: '' }]) });
    const map = byWire(ds);
    expect([...map.keys()].sort()).toEqual([0, 1, 4]);
    expect(worst(map.get(0)!)).toBe('error');
    expect(worst(map.get(1)!)).toBe('warning');
    expect(worst([])).toBeNull();
  });
});
