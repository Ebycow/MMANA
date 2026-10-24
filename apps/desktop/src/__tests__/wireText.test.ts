import { describe, expect, it } from 'vitest';
import type { Wire } from '../api/types';
import { wiresFromText, wiresToText } from '../domain/wireText';

const wire = (x: number, radius: number, segmentation: Wire['segmentation'] = { kind: 'auto' }): Wire => ({
  start: { x, y: 0.6, z: 0 },
  end: { x, y: -2, z: 0 },
  radius,
  segmentation,
});

describe('wires as .maa wire lines', () => {
  it('writes the wire lines of a .maa file', () => {
    expect(wiresToText([wire(-1.6, 0.007), wire(-1.3, 8e-4, { kind: 'taperCustom', dm1: 400, dm2: 40 })])).toBe(
      '-1.6,\t0.6,\t0.0,\t-1.6,\t-2.0,\t0.0,\t0.007,\t0\r\n' + '-1.3,\t0.6,\t0.0,\t-1.3,\t-2.0,\t0.0,\t8.000e-04,\t400,40\r\n',
    );
  });

  it('reads back what it writes', () => {
    const ws = [wire(-1.6, 0.007, { kind: 'fixed', count: 11 }), wire(2.5, 8e-4, { kind: 'taperCustom', dm1: 400, dm2: 40 }), wire(0, -1, { kind: 'taper' })];
    expect(wiresFromText(wiresToText(ws))).toEqual(ws);
  });

  it('takes commas, tabs or spaces and a missing SEG', () => {
    expect(wiresFromText('0 0 0 1 0 0 0.001\n\n0\t0\t0\t0\t1\t0\t0.001\t-1\n')).toEqual([
      { start: { x: 0, y: 0, z: 0 }, end: { x: 1, y: 0, z: 0 }, radius: 0.001, segmentation: { kind: 'auto' } },
      { start: { x: 0, y: 0, z: 0 }, end: { x: 0, y: 1, z: 0 }, radius: 0.001, segmentation: { kind: 'taper' } },
    ]);
  });

  it.each([
    ['empty text', ' \n'],
    ['a sentence', 'hello world'],
    ['a short line', '0, 0, 0, 1, 0, 0'],
    ['a non-number', '0, 0, 0, 1, 0, x, 0.001, 0'],
    ['a fractional SEG', '0, 0, 0, 1, 0, 0, 0.001, 1.5'],
    ['one bad line among wires', '0, 0, 0, 1, 0, 0, 0.001, 0\n2'],
  ])('rejects %s', (_, text) => {
    expect(wiresFromText(text)).toBeNull();
  });
});
