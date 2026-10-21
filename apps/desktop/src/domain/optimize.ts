// Optimisation (MMANA's 最適化): labels, the default setup, the 連携 and
// Min / Max cells as text, and the order of the trial list. The search and
// the setup edits run in mmana-core (`optimize.rs`); the UI sends the setup
// and shows what comes back.
import type {
  ElementRow,
  Link,
  LoadKind,
  OptimizerSetup,
  PatternCut,
  SetupIssue,
  Trial,
  Variable,
  VariableTarget,
  Weights,
} from '../api/types';
import { evaluate } from './expr';
import { strDbl } from './elements';

export type TargetKind = VariableTarget['kind'];

/** The 種類 menu, in MMANA's order. */
export const TARGET_KINDS: { kind: TargetKind; label: string }[] = [
  { kind: 'wire', label: 'ワイヤ' },
  { kind: 'polarWire', label: 'ワイヤ（極座標）' },
  { kind: 'element', label: 'エレメント' },
  { kind: 'load', label: '集中定数' },
  { kind: 'height', label: '地上高' },
  { kind: 'frequency', label: '周波数' },
  { kind: 'source', label: '給電点' },
  { kind: 'stack', label: 'スタック間隔' },
];

export const kindLabel = (k: TargetKind) => TARGET_KINDS.find((t) => t.kind === k)?.label ?? k;

const WIRE_PARTS = ['y1', 'z1', 'y2', 'z2', 'radius', 'x1', 'x2'] as const;
const WIRE_LABELS = ['Y1', 'Z1', 'Y2', 'Z2', 'R', 'X1', 'X2'];
const QUANTITIES = ['length', 'azimuth', 'zenith'] as const;
const ANCHORS = ['start', 'end', 'center'] as const;
const POLAR_LABELS = ['長さ', '径度', '天頂角'];
const ANCHOR_LABELS = ['始点', '終点', '中心'];
const ELEMENT_PARTS = ['interval', 'width', 'height', 'perimeter', 'radius', 'position', 'widthAll', 'heightAll', 'perimeterAll'] as const;
const STACK_PARTS = ['both', 'horizontal', 'vertical'] as const;

/** What the 箇所 column needs to know about the thing a variable varies. */
export interface PartContext {
  /** The element's perimeter column is its X width (Ｘ幅). */
  xWidth?: boolean;
  loadKind?: LoadKind['kind'];
}

/** The 箇所 choices of a kind, by index (the original's `SubPos`). */
export function partLabels(kind: TargetKind, ctx: PartContext = {}): string[] {
  switch (kind) {
    case 'wire':
      return WIRE_LABELS;
    case 'polarWire':
      return POLAR_LABELS.flatMap((q) => ANCHOR_LABELS.map((a) => `${q}（${a}基準）`));
    case 'element': {
      const p = ctx.xWidth ? 'Ｘ幅' : '全周';
      return ['間隔', '横幅', '縦幅', p, 'R', '位置', '横幅（全座標）', '縦幅（全座標）', `${p}（全座標）`];
    }
    case 'load':
      return ctx.loadKind === 'impedance' ? ['R', 'jX'] : ['L', 'C'];
    case 'source':
      return ['位相', '電圧'];
    case 'stack':
      return ['間隔', '水平間隔', '垂直間隔'];
    default:
      return [];
  }
}

/** The original's `SubPos`. */
export function partIndex(t: VariableTarget): number {
  switch (t.kind) {
    case 'wire':
      return WIRE_PARTS.indexOf(t.part);
    case 'polarWire':
      return QUANTITIES.indexOf(t.quantity) * 3 + ANCHORS.indexOf(t.anchor);
    case 'element':
      return ELEMENT_PARTS.indexOf(t.part);
    case 'load':
      return t.part === 'secondary' ? 1 : 0;
    case 'source':
      return t.part === 'voltage' ? 1 : 0;
    case 'stack':
      return STACK_PARTS.indexOf(t.part);
    default:
      return 0;
  }
}

/** The wire, element, load or source (0-based), or null for the height, frequency and stacking. */
export function targetPosition(t: VariableTarget): number | null {
  switch (t.kind) {
    case 'wire':
    case 'polarWire':
      return t.wire;
    case 'element':
      return t.element;
    case 'load':
      return t.load;
    case 'source':
      return t.source;
    default:
      return null;
  }
}

/** A target from the three columns; a part the kind lacks becomes its first. */
export function makeTarget(kind: TargetKind, position: number, part: number): VariableTarget {
  const pick = <T,>(list: readonly T[]) => list[part] ?? list[0];
  switch (kind) {
    case 'wire':
      return { kind, wire: position, part: pick(WIRE_PARTS) };
    case 'polarWire': {
      const p = part >= 0 && part < 9 ? part : 0;
      return { kind, wire: position, quantity: QUANTITIES[Math.floor(p / 3)], anchor: ANCHORS[p % 3] };
    }
    case 'element':
      return { kind, element: position, part: pick(ELEMENT_PARTS) };
    case 'load':
      return { kind, load: position, part: part === 1 ? 'secondary' : 'primary' };
    case 'source':
      return { kind, source: position, part: part === 1 ? 'voltage' : 'phase' };
    case 'stack':
      return { kind, part: pick(STACK_PARTS) };
    default:
      return { kind };
  }
}

/** Unit of a variable's value. */
export function targetUnit(t: VariableTarget, loadKind?: LoadKind['kind']): string {
  switch (t.kind) {
    case 'polarWire':
      return t.quantity === 'length' ? 'm' : '°';
    case 'load':
      if (loadKind === 'impedance') return 'Ω';
      return t.part === 'secondary' ? 'pF' : 'µH';
    case 'frequency':
      return 'MHz';
    case 'source':
      return t.part === 'voltage' ? 'V' : '°';
    default:
      return 'm';
  }
}

export function defaultSetup(): OptimizerSetup {
  return {
    variables: [],
    weights: { gain: 50, fb: 50, jx: 50, swr: 0, elevation: 0, matching: 0, current: 0 },
    sweepOnly: false,
    absolutePitch: true,
    coarsePattern: true,
    goals: {
      maxGainDbi: 100,
      maxFbDb: 25,
      minJx: 0,
      minSwr: 1,
      matching: { kind: 'capacitance' },
      currentPulse: 'w1c',
      minimizeCurrent: false,
    },
    bands: [],
    emphasizeFirstBand: true,
  };
}

export const WEIGHT_LABELS: { key: keyof Weights; label: string }[] = [
  { key: 'gain', label: 'Gain' },
  { key: 'fb', label: 'F/B' },
  { key: 'jx', label: 'jX' },
  { key: 'swr', label: 'SWR' },
  { key: 'elevation', label: '打ち上げ角' },
  { key: 'matching', label: 'マッチング回路' },
  { key: 'current', label: '電流' },
];

/** `SetTradeStr`: each goal's share of the score. */
export function tradeOffText(setup: OptimizerSetup): string {
  if (setup.sweepOnly) return '目標なし（単純スイープ）';
  const w = setup.weights;
  const total = WEIGHT_LABELS.reduce((s, { key }) => s + w[key], 0);
  const parts = WEIGHT_LABELS.filter(({ key }) => total === 0 || w[key] > 0).map(({ key, label }) => {
    const share = total === 0 ? 100 / WEIGHT_LABELS.length : (w[key] * 100) / total;
    return `${label} ${share.toFixed(1)}%`;
  });
  return parts.join('　');
}

// ------------------------------------------------------------- 連携 cell ---

/** `GetWithStr`: `0`, `3`, `-3`, `3+1.5`, `-3+1.5`, `3*1.05`, `3/2` (1-based). */
export function formatLink(link: Link | undefined): string {
  if (!link || link.kind === 'independent') return '0';
  const n = link.variable + 1;
  const signed = (c: number) => `${c >= 0 ? '+' : '-'}${strDbl(Math.abs(c))}`;
  switch (link.kind) {
    case 'same':
      return `${n}`;
    case 'negated':
      return `-${n}`;
    case 'offset':
      return `${n}${signed(link.offset)}`;
    case 'negatedOffset':
      return `-${n}${signed(link.offset)}`;
    case 'scaled':
      return `${n}*${strDbl(link.factor)}`;
    case 'divided':
      return `${n}/${strDbl(link.divisor)}`;
  }
}

/**
 * `SetWithValue`: reads the 連携 cell of variable `row`. `=` in place of the
 * number takes it from the current values (`2+=` keeps today's difference).
 * Returns null for text that is not a link.
 */
export function parseLink(text: string, row: number, values: (number | null | undefined)[]): Link | null {
  const t = text.replace(/[！-～]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/\s+/g, '');
  const m = /^(-?\d+)(?:([+\-*/])(.+))?$/.exec(t);
  if (!m) return null;
  const n = Number(m[1]);
  if (n === 0) return m[2] ? null : { kind: 'independent' };
  const variable = Math.abs(n) - 1;
  const negated = n < 0;
  if (!m[2]) return negated ? { kind: 'negated', variable } : { kind: 'same', variable };
  const op = m[2];
  const auto = m[3] === '=';
  const own = values[row];
  const followed = values[variable];
  let c: number;
  if (auto) {
    if (own == null || followed == null) return null;
    // (a product or quotient follows the variable itself, as in MMANA)
    if (op === '+' || op === '-') c = own - (negated ? -followed : followed);
    else if (op === '*') c = followed !== 0 ? own / followed : 0;
    else c = own !== 0 ? followed / own : 0;
  } else {
    const v = evaluate(m[3], { wavelength: 1 });
    if (v == null) return null;
    c = op === '-' ? -v : v;
  }
  if (op === '+' || op === '-') {
    if (c === 0) return negated ? { kind: 'negated', variable } : { kind: 'same', variable };
    return negated ? { kind: 'negatedOffset', variable, offset: c } : { kind: 'offset', variable, offset: c };
  }
  // -n*c is n*(-c)
  if (negated && !auto) c = -c;
  if (c === 0) c = 1;
  if (c === 1) return { kind: 'same', variable };
  if (c === -1) return { kind: 'negated', variable };
  return op === '*' ? { kind: 'scaled', variable, factor: c } : { kind: 'divided', variable, divisor: c };
}

// ------------------------------------------------------- Min / Max cells ---

/** `#3` (the value of variable 3) or the number. */
export function formatBound(v: Variable, which: 'min' | 'max'): string {
  const from = which === 'min' ? v.minFrom : v.maxFrom;
  return from != null ? `#${from + 1}` : strDbl(which === 'min' ? v.min : v.max);
}

export function parseBound(text: string): { value: number } | { from: number } | null {
  const t = text.trim();
  const m = /^[#＃](\d+)$/.exec(t);
  if (m) return Number(m[1]) >= 1 ? { from: Number(m[1]) - 1 } : null;
  const v = evaluate(t, { wavelength: 1 });
  return v == null ? null : { value: v };
}

export function withBound(v: Variable, which: 'min' | 'max', b: { value: number } | { from: number }): Variable {
  const from = 'from' in b ? b.from : null;
  return which === 'min'
    ? { ...v, minFrom: from, min: 'value' in b ? b.value : v.min }
    : { ...v, maxFrom: from, max: 'value' in b ? b.value : v.max };
}

// ---------------------------------------------------------------- issues ---

export function issueMessage(i: SetupIssue): string {
  const v = i.variable != null ? `変数 ${i.variable + 1}: ` : '';
  const other = i.other != null ? i.other + 1 : 0;
  switch (i.code) {
    case 'noVariables':
      return '可変パラメータがありません。';
    case 'tooManyVariables':
      return '変数は 128 個までです。';
    case 'missingTarget':
      return `${v}この変数は存在しません。`;
    case 'zeroValue':
      return `${v}現在値が 0 なので可変できません。`;
    case 'sDomainLoad':
      return `${v}S パラメータ形式の集中定数は可変できません。`;
    case 'heightInFreeSpace':
      return `${v}自由空間なので地上高を変化させても意味はありません。`;
    case 'duplicate':
      return `${v}変数 ${other} と重複しています。`;
    case 'overlapping':
      return `${v}変数 ${other} と同じ座標や値を動かすため、動かした順でアンテナが変わります（試行の値から計算したアンテナに戻せません）。どちらかを外してください。`;
    case 'missingVariable':
      return `${v}変数 ${other} は存在しません。`;
    case 'linkLoop':
      return `${v}連携先の独立変数が見つかりません（連携が循環しています）。`;
    case 'tooManyBands':
      return 'バンドは 10 個までです。';
    case 'bandWithoutSources':
      return `バンド ${other} に給電点がありません。`;
    case 'bandMissingSource':
      return `${v}バンド ${other} にはこの給電点がありません（バンドごとに給電点が設定し直されるため、すべてのバンドに必要です）。`;
    case 'linkValueDiffers':
      return `${v}連携先の変数 ${other} と初期値が異なります。実行すると連携先の値に合わせます。`;
  }
}

/** An element has two or more of width, height and X width, which "all elements" asks about. */
export const needsCombination = (rows: ElementRow[]) =>
  rows.some((r) => r.xWidth && [r.width, r.height, r.perimeter].filter((d) => d !== 0).length >= 2);

// ---------------------------------------------------------------- trials ---

export type TrialSort = 'score' | 'jx' | 'swr' | 'gain' | 'fb' | 'elevation';

export const TRIAL_SORTS: { value: TrialSort; label: string }[] = [
  { value: 'score', label: '評価値' },
  { value: 'jx', label: 'jX' },
  { value: 'swr', label: 'SWR' },
  { value: 'gain', label: 'Gain' },
  { value: 'fb', label: 'F/B' },
  { value: 'elevation', label: '打ち上げ角' },
];

/** The trial list's 順序: best first. */
export function sortTrials(trials: Trial[], by: TrialSort): Trial[] {
  const key: Record<TrialSort, (t: Trial) => number> = {
    score: (t) => -t.score,
    jx: (t) => Math.abs(t.impedance.im),
    swr: (t) => t.swr,
    gain: (t) => -t.gainDbi,
    fb: (t) => -t.fbDb,
    elevation: (t) => t.elevationDeg,
  };
  return [...trials].sort((a, b) => key[by](a) - key[by](b));
}

/** `RoundUpStr`: six decimals. */
const roundUpStr = (d: number) => Number(d.toFixed(6));

/** `GetCurIndex`: the trial the antenna is at now, or 0. */
export function currentTrial(trials: Trial[], values: (number | null | undefined)[]): number {
  const i = trials.findIndex((t) => t.values.every((v, k) => values[k] != null && roundUpStr(v) === roundUpStr(values[k]!)));
  return Math.max(0, i);
}

/** A trial's pattern (total gain every 2°) as a cut the polar plot draws. */
export const trialCut = (total: number[]): PatternCut => ({
  startDeg: 0,
  stepDeg: 2,
  total,
  verticalPol: [],
  horizontalPol: [],
});

/** `AutoJXClick`: the jX that, with R, an L network matches to `target` Ω. */
export function matchingJx(r: number, target: number): number | null {
  if (!(r > 0)) return null;
  if (target === r) return 0;
  if (target > r) return -(Math.sqrt(target / r - 1) * r);
  return null;
}
