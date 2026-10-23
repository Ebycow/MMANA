// Names a step of the undo history from what it changed, so the history list
// reads without every edit naming itself. Edits that mean more than their
// difference (an optimisation, the element editor) name themselves.
import type { Antenna, Environment, Project } from '../api/types';

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const ANTENNA_PARTS: [keyof Antenna, string][] = [
  ['sources', '給電点を変更'],
  ['autoVoltage', '給電点を変更'],
  ['loads', '負荷を変更'],
  ['loadsEnabled', '負荷を変更'],
  ['autoSegmentation', '自動分割の設定を変更'],
  ['stack', 'スタックを変更'],
  ['combinedWires', '組み合わせワイヤーを変更'],
  ['comment', 'コメントを変更'],
];

const ENVIRONMENT_PARTS: [keyof Environment, string][] = [
  ['ground', '大地の設定を変更'],
  ['realGround', '大地の設定を変更'],
  ['height', '地上高を変更'],
  ['wireMaterial', 'ワイヤーの材質を変更'],
  ['swrReference', 'SWR の基準インピーダンスを変更'],
  ['fbRange', 'F/B の範囲を変更'],
];

function wireChange(before: Antenna['wires'], after: Antenna['wires']): string | null {
  if (after.length > before.length) return `ワイヤーを ${after.length - before.length} 本追加`;
  if (after.length < before.length) return `ワイヤーを ${before.length - after.length} 本削除`;
  const changed = after.flatMap((w, i) => (same(w, before[i]) ? [] : [i + 1]));
  if (changed.length === 0) return null;
  return changed.length <= 3 ? `ワイヤー ${changed.join('、')} を変更` : `ワイヤー ${changed.length} 本を変更`;
}

/** What changed from `before` to `after`, in a few words. */
export function describeChange(before: Project, after: Project): string {
  const a = before.antenna;
  const b = after.antenna;
  const parts = [
    a.name !== b.name ? `アンテナ名を「${b.name}」に変更` : null,
    a.frequencyMhz !== b.frequencyMhz ? `設計周波数を ${b.frequencyMhz} MHz に変更` : null,
    wireChange(a.wires, b.wires),
    ...ANTENNA_PARTS.map(([key, label]) => (same(a[key], b[key]) ? null : label)),
    ...ENVIRONMENT_PARTS.map(([key, label]) => (same(before.environment[key], after.environment[key]) ? null : label)),
  ];
  const unique = [...new Set(parts.filter((p) => p != null))];
  if (unique.length === 0) return '変更なし';
  return unique.length <= 2 ? unique.join('、') : `${unique.slice(0, 2).join('、')} など`;
}
