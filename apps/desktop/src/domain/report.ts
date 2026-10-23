// What a bug report needs, as Markdown to paste into a GitHub issue: the
// build, the result and problems the user saw, the last errors and the
// antenna itself. The user reads it before pasting, so it says only what it
// shows.
import type { CalcResult, Project } from '../api/types';
import type { EngineError } from '../api/transport';
import type { LoggedMessage, MessageLevel } from '../state/messageLog';
import type { Diagnostic, Severity } from './diagnostics';

export interface ReportSources {
  /** The version line, e.g. MMANA v0.2.0-43-g060248a (Web, built …). */
  version: string;
  userAgent: string;
  project: Project;
  result: CalcResult | null;
  /** The antenna changed since `result`. */
  resultStale: boolean;
  /** The error of the last calculation, when it was of the antenna as it is. */
  calcError: EngineError | null;
  /** Every problem found, hidden ones too. */
  diagnostics: Diagnostic[];
  /** The messages shown lately, oldest first. */
  messages: readonly LoggedMessage[];
  includeAntenna: boolean;
}

const SEVERITY: Record<Severity, string> = { error: 'エラー', warning: '警告', info: '情報' };
const LEVEL: Record<MessageLevel, string> = { error: 'エラー', warning: '注意', info: 'お知らせ' };

const z = (re: number, im: number) => `${re.toFixed(2)} ${im < 0 ? '−' : '+'} j${Math.abs(im).toFixed(2)}`;
const pad = (n: number) => String(n).padStart(2, '0');
const time = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
/** Indented JSON with each innermost object or array on one line, so a wire takes a few lines, not twenty. */
export function compactJson(value: unknown, indent = ''): string {
  if (value === null || typeof value !== 'object') return JSON.stringify(value) ?? 'null';
  const array = Array.isArray(value);
  const entries: [string | null, unknown][] = array
    ? value.map((v) => [null, v])
    : Object.entries(value).filter(([, v]) => v !== undefined);
  const [open, close] = array ? ['[', ']'] : ['{', '}'];
  if (entries.length === 0) return open + close;
  const inner = indent + '  ';
  const items = entries.map(([k, v]) => (k === null ? '' : `${JSON.stringify(k)}: `) + compactJson(v, inner));
  if (entries.every(([, v]) => v === null || typeof v !== 'object')) return `${open} ${items.join(', ')} ${close}`;
  return `${open}\n${items.map((i) => inner + i).join(',\n')}\n${indent}${close}`;
}

const list = (lines: string[]) => (lines.length ? lines.map((l) => `- ${l}`).join('\n') : 'なし');

function resultLines({ project, result, resultStale, calcError }: ReportSources): string {
  if (calcError) return `計算できませんでした：${calcError.title}（${calcError.detail}）`;
  if (!result) return 'まだ計算していません';
  const ref = project.environment.swrReference;
  const lines = [
    `周波数 ${result.frequencyMhz.toFixed(3)} MHz`,
    `インピーダンス ${z(result.impedance.re, result.impedance.im)} Ω`,
    `SWR ${result.swr >= 99999 ? '∞' : result.swr.toFixed(2)}（${ref.x === 0 ? ref.r : z(ref.r, ref.x)} Ω）`,
    `利得 ${result.gainDbi.toFixed(2)} dBi、F/B ${result.fbDb.toFixed(2)} dB、打ち上げ角 ${result.elevationDeg.toFixed(1)}°`,
    // the warnings about the whole antenna; those naming wires are among the problems
    ...result.warnings.filter((w) => w.wires.length === 0).map((w) => `注意：${w.message}`),
  ];
  return (resultStale ? 'アンテナを変更した後、再計算していません。\n\n' : '') + list(lines);
}

export function bugReport(s: ReportSources): string {
  const sections = [
    `### 環境\n\n${list([s.version, s.userAgent])}`,
    `### 計算結果\n\n${resultLines(s)}`,
    `### 警告\n\n${list(s.diagnostics.map((d) => `${SEVERITY[d.severity]}：${d.message}`))}`,
    `### 直近のメッセージ\n\n${list(s.messages.map((m) => `${time(m.at)} [${LEVEL[m.level]}] ${[m.title, m.detail.replace(/\s*\n\s*/g, ' ')].filter(Boolean).join('：')}`))}`,
  ];
  if (s.includeAntenna) {
    sections.push(
      [
        '<details><summary>アンテナ（プロジェクト JSON。.json として保存すると MMANA で開けます）</summary>',
        '',
        '```json',
        compactJson(s.project),
        '```',
        '',
        '</details>',
      ].join('\n'),
    );
  }
  return sections.join('\n\n') + '\n';
}
