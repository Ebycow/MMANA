// MMANA's SEG column: the text users type and its structured form.
// Mirrors `Segmentation::to_code` / `from_code` in mmana-core.
import type { Segmentation } from '../api/types';

export function segmentationToCode(s: Segmentation): number {
  switch (s.kind) {
    case 'fixed':
      return s.count;
    case 'auto':
      return 0;
    case 'taper':
      return -1;
    case 'taperStart':
      return -2;
    case 'taperEnd':
      return -3;
    case 'taperCustom':
      return s.dm2 == null
        ? -Math.abs(s.dm1)
        : (0x80000000 | (Math.abs(s.dm1) & 0x7fff) | ((Math.abs(s.dm2) & 0x7fff) << 16)) | 0;
  }
}

export function segmentationFromCode(code: number): Segmentation {
  if (code > 0) return { kind: 'fixed', count: code };
  if (code === 0) return { kind: 'auto' };
  if (code === -1) return { kind: 'taper' };
  if (code === -2) return { kind: 'taperStart' };
  if (code === -3) return { kind: 'taperEnd' };
  if (code < -10000) return { kind: 'taperCustom', dm1: code & 0x7fff, dm2: (code >> 16) & 0x7fff };
  return { kind: 'taperCustom', dm1: -code, dm2: null };
}

/** `Seg2Str` */
export function formatSegmentation(s: Segmentation): string {
  if (s.kind === 'taperCustom' && s.dm2 != null) return `${s.dm1},${s.dm2}`;
  return String(segmentationToCode(s));
}

/** `Str2Seg`; returns null when the text is not a valid SEG value. */
export function parseSegmentation(text: string): Segmentation | null {
  const t = text.trim();
  const pair = /^(-?\d+)\s*,\s*(-?\d+)$/.exec(t);
  if (pair) {
    const dm1 = Math.abs(Number(pair[1]));
    const dm2 = Math.abs(Number(pair[2]));
    if (dm1 && dm2 && dm1 > dm2) return { kind: 'taperCustom', dm1, dm2 };
    return null;
  }
  if (/^-?\d+$/.test(t)) return segmentationFromCode(Number(t));
  return null;
}

export function describeSegmentation(s: Segmentation): string {
  switch (s.kind) {
    case 'fixed':
      return `${s.count} 等分`;
    case 'auto':
      return '自動 (等分割)';
    case 'taper':
      return '自動 (両端テーパ)';
    case 'taperStart':
      return '自動 (始点テーパ)';
    case 'taperEnd':
      return '自動 (終点テーパ)';
    case 'taperCustom':
      return s.dm2 == null ? `テーパ λ/${s.dm1}` : `テーパ λ/${s.dm1}〜λ/${s.dm2}`;
  }
}
