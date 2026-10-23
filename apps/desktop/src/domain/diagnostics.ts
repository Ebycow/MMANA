// Problems with the antenna, gathered from every check into one list by
// rule, so the wire table and the 3D view can mark the wires they are about.
// The engine reports what stopped a calculation (one problem) and what makes
// a result doubtful; the connection check finds wires that touch but are not
// joined, without calculating.
import type { CalcResult, Wire } from '../api/types';
import type { EngineError } from '../api/transport';
import { describeIssue, findJunctionIssues, issueWires, type JunctionIssue } from './junctions';

/** error: the antenna cannot be calculated; warning: the result is doubtful; info: often intended. */
export type Severity = 'error' | 'warning' | 'info';

export type RuleId =
  | 'overlap'
  | 'zero-length'
  | 'combined-wire'
  | 'segmentation'
  | 'engine-error'
  | 'tee'
  | 'gap'
  | 'too-many-segments'
  | 'below-ground'
  | 'cross';

export interface Rule {
  id: RuleId;
  severity: Severity;
  label: string;
  description: string;
}

/** Every rule, errors first. Errors cannot be hidden: the calculation stops on them. */
export const RULES: Rule[] = [
  { id: 'overlap', severity: 'error', label: '重なったワイヤー', description: '両端が一致するワイヤーが 2 本あると計算できません。' },
  { id: 'zero-length', severity: 'error', label: '長さ 0 のワイヤー', description: '始点と終点が同じワイヤーは計算できません。' },
  { id: 'combined-wire', severity: 'error', label: '組み合わせワイヤーの定義がない', description: '負の半径に対応する組み合わせワイヤーの定義がありません。' },
  { id: 'segmentation', severity: 'error', label: '分割数の誤り', description: 'SEG が 1 未満か、テーパの値が範囲外です。' },
  { id: 'engine-error', severity: 'error', label: 'その他の計算エラー', description: '計算エンジンがワイヤーを名指しして計算を止めました。' },
  {
    id: 'tee',
    severity: 'warning',
    label: 'ワイヤーの途中に触れた端',
    description: '端が別のワイヤーの途中に触れていますが、計算上はつながっていません。',
  },
  { id: 'gap', severity: 'warning', label: 'わずかにずれた端', description: '触れているように見える端どうしの座標がずれていて、計算上はつながっていません。' },
  {
    id: 'too-many-segments',
    severity: 'warning',
    label: '分割しすぎた導体',
    description: '1 本の導体を約 250 より多く分割すると、分割数によって結果が変わります。',
  },
  { id: 'below-ground', severity: 'warning', label: '地面より下のワイヤー', description: '大地があるとき、地面より下にあるワイヤーは正しく計算されません。' },
  {
    id: 'cross',
    severity: 'info',
    label: '途中で交わるワイヤー',
    description: 'つながっていない交差です。たすき掛けの位相線（HB9CV・LPDA など）では意図どおりです。',
  },
];

export const ruleOf = (id: RuleId): Rule => RULES.find((r) => r.id === id)!;

/** Whether the viewer may hide a rule (errors stop the calculation, so they always show). */
export const canHide = (id: RuleId) => ruleOf(id).severity !== 'error';

export interface Diagnostic {
  rule: RuleId;
  severity: Severity;
  /** 0-based indices of the wires it is about. */
  wires: number[];
  message: string;
}

const diagnostic = (rule: RuleId, wires: number[], message: string): Diagnostic => ({
  rule,
  severity: ruleOf(rule).severity,
  wires: [...new Set(wires)].sort((a, b) => a - b),
  message,
});

const JUNCTION_RULE: Record<JunctionIssue['kind'], RuleId> = { overlap: 'overlap', tee: 'tee', gap: 'gap', cross: 'cross' };

const ERROR_RULE: Record<string, RuleId> = {
  'overlapping-wires': 'overlap',
  'zero-length-wire': 'zero-length',
  'combined-wire-not-found': 'combined-wire',
  'invalid-segment-count': 'segmentation',
  'invalid-wire-segmentation': 'segmentation',
};

const wireList = (wires: number[]) => wires.map((w) => w + 1).join('、');

/** The problem a calculation error names wires for; null when it names none. */
export function errorDiagnostic(error: EngineError): Diagnostic | null {
  const wires = error.wires.map((w) => w - 1);
  if (wires.length === 0) return null;
  const rule = (error.code && ERROR_RULE[error.code]) || 'engine-error';
  switch (rule) {
    case 'overlap':
      return diagnostic(rule, wires, `ワイヤー ${wires[0] + 1} と ${wires[1] + 1} が重なっています`);
    case 'zero-length':
      return diagnostic(rule, wires, `ワイヤー ${wireList(wires)} の長さが 0 です`);
    default:
      return diagnostic(rule, wires, `ワイヤー ${wireList(wires)}：${error.detail}`);
  }
}

const WARNING_RULE: Partial<Record<CalcResult['warnings'][number]['code'], RuleId>> = {
  tooManySegments: 'too-many-segments',
  belowGround: 'below-ground',
};

/** The problems of a result that name wires (the others are about the whole antenna). */
export function resultDiagnostics(result: CalcResult): Diagnostic[] {
  return result.warnings.flatMap((w) => {
    const rule = WARNING_RULE[w.code];
    if (!rule || w.wires.length === 0) return [];
    const wires = w.wires.map((n) => n - 1);
    const message =
      rule === 'too-many-segments'
        ? `ワイヤー ${wireList(wires)} がつながった導体の分割が多すぎます（約 250 を超えると結果が分割数で変わります）`
        : `ワイヤー ${wireList(wires)} が地面より下にあります`;
    return [diagnostic(rule, wires, message)];
  });
}

export interface DiagnosticSources {
  wires: Wire[];
  /** The error of the last calculation, when it was of the antenna as it is now. */
  error: EngineError | null;
  /** The last result, when it is of the antenna as it is now. */
  result: CalcResult | null;
}

/** Every problem found, without duplicates (the engine and the connection check both find overlaps), errors first. */
export function collectDiagnostics({ wires, error, result }: DiagnosticSources): Diagnostic[] {
  const all = [
    ...(error ? [errorDiagnostic(error)].filter((d) => d != null) : []),
    ...findJunctionIssues(wires).map((i) => diagnostic(JUNCTION_RULE[i.kind], issueWires(i), describeIssue(i))),
    ...(result ? resultDiagnostics(result) : []),
  ];
  const seen = new Set<string>();
  const unique = all.filter((d) => {
    const key = `${d.rule}:${d.wires.join(',')}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  const order = RULES.map((r) => r.id);
  return unique.sort((a, b) => order.indexOf(a.rule) - order.indexOf(b.rule));
}

/** The problems the viewer has not hidden. */
export const shown = (diagnostics: Diagnostic[], hidden: readonly RuleId[]) => diagnostics.filter((d) => !canHide(d.rule) || !hidden.includes(d.rule));

/** The problems of each wire (0-based), errors first. */
export function byWire(diagnostics: Diagnostic[]): Map<number, Diagnostic[]> {
  const map = new Map<number, Diagnostic[]>();
  for (const d of diagnostics) for (const w of d.wires) map.set(w, [...(map.get(w) ?? []), d]);
  return map;
}

const RANK: Record<Severity, number> = { error: 0, warning: 1, info: 2 };

/** The most severe of some problems. */
export const worst = (diagnostics: Diagnostic[]): Severity | null =>
  diagnostics.reduce<Severity | null>((w, d) => (w == null || RANK[d.severity] < RANK[w] ? d.severity : w), null);
