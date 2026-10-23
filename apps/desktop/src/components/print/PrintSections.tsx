// The sections of the printed sheets and the heights they are laid out with.
// Each renders into a box of exactly that height, so the preview is the page.
import type { ReactNode } from 'react';
import type { CalcResult, ElementEdit, GroundKind, Load, Point3, Project, WireMaterial } from '../../api/types';
import type { EngineError } from '../../api/transport';
import type { PolarScale } from '../../charts/polar';
import type { Diagnostic } from '../../domain/diagnostics';
import { feedPositions } from '../../domain/feed';
import { displayWires } from '../../domain/geometry';
import {
  KIND_LABELS,
  lengthDigits,
  lengthToDisplay,
  lengthUnit,
  radiusDigits,
  radiusToDisplay,
  radiusUnit,
  type Units,
} from '../../domain/elements';
import { HEADING, ROW_HEIGHT, TABLE_HEAD, extentOf, swrAgainst, swrBandwidth, type PrintBlock, type SectionId } from '../../domain/print';
import { wavelength, wireLength } from '../../domain/project';
import { formatSegmentation } from '../../domain/segmentation';
import { add, sub } from '../../domain/wireEdit';
import type { ResonanceState, SweepState } from '../../state/store';
import { format } from '../Cells';
import { LineSvg, PolarSvg, PRINT_SERIES, ShapeViews, layoutViews, scaleText } from './PrintCharts';

/** Everything the sheets show, gathered when the preview opens. */
export interface PrintData {
  project: Project;
  fileName: string;
  version: string;
  printedAt: Date;
  /** The last result, null before any calculation. */
  result: CalcResult | null;
  /** The antenna changed since `result`. */
  resultStale: boolean;
  calcError: EngineError | null;
  calcFrequencyMhz: number;
  units: Units;
  polarScale: PolarScale;
  diagnostics: Diagnostic[];
  elements: ElementEdit | null;
  sweep: SweepState;
  resonance: ResonanceState;
}

/** The content width of a sheet, in mm. */
const WIDTH = 186;

const GROUNDS: Record<GroundKind, string> = { freeSpace: '自由空間', perfect: '完全大地', real: 'リアルグランド' };
const MATERIALS: Record<WireMaterial, string> = {
  lossless: '無損失',
  copper: '銅線',
  copperPipe: '銅パイプ',
  aluminum: 'アルミ線',
  aluminumPipe: 'アルミパイプ',
  iron: '鉄線',
  ironPipe: '鉄パイプ',
};

const z = (re: number, im: number, digits = 2) => `${re.toFixed(digits)} ${im < 0 ? '−' : '+'} j${Math.abs(im).toFixed(digits)}`;
const swrText = (v: number) => (!Number.isFinite(v) || v >= 99999 ? '∞' : v.toFixed(2));
const db = (v: number) => (v < -999 ? '−999' : v.toFixed(2));
const pad = (n: number) => String(n).padStart(2, '0');
export const dateText = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;

const length = (m: number, u: Units) => format(lengthToDisplay(m, u), lengthDigits(u));
const lengthWithUnit = (m: number, u: Units) => `${length(m, u)} ${lengthUnit(u)}`;

/** The result when it is of the antenna as it is. */
const current = (d: PrintData) => (d.result && !d.resultStale ? d.result : null);

const Heading = ({ children, note }: { children: ReactNode; note?: ReactNode }) => (
  <div className="p-heading">
    <span>{children}</span>
    {note && <span className="p-heading-note">{note}</span>}
  </div>
);

// ---------------------------------------------------------------- title ---

const commentLines = (d: PrintData) => Math.min(4, (d.project.antenna.comment ?? '').split('\n').filter((l) => l.trim()).length);

function Title({ d }: { d: PrintData }) {
  const a = d.project.antenna;
  const comment = (a.comment ?? '').trim();
  return (
    <div className="p-title-block">
      <div className="p-antenna-name">{a.name || '（名前なし）'}</div>
      <div className="p-subtitle">
        <span>{d.fileName}</span>
        <span>設計周波数 {a.frequencyMhz} MHz</span>
        {d.calcFrequencyMhz !== a.frequencyMhz && <span>計算周波数 {d.calcFrequencyMhz} MHz</span>}
        <span>波長 {wavelength(a.frequencyMhz).toFixed(3)} m</span>
      </div>
      {comment && <div className="p-comment">{comment}</div>}
    </div>
  );
}

// -------------------------------------------------------------- summary ---

/** The lines MMANA printed under the vertical pattern (`CRes::DrawRes`). */
function mmanaLines(r: CalcResult, p: Project): string[] {
  const env = p.environment;
  const vertical = r.maxVerticalPolDbi > r.maxHorizontalPolDbi;
  const back = env.fbRange.vertical ? `水平 ${env.fbRange.horizontal}° 垂直 ${env.fbRange.vertical}°` : `水平 ${env.fbRange.horizontal}°`;
  const ref = env.swrReference;
  const ground = env.ground === 'freeSpace' ? GROUNDS.freeSpace : `${GROUNDS[env.ground]}, ${env.height} mH`;
  const lines = [
    `Ga  : ${db(r.gainDbi)} dBi = 0 dB（${vertical ? '垂直' : '水平'}偏波）`,
    ...(env.ground === 'freeSpace' ? [`Gh  : ${db(r.gainDbi - 2.15)} dBd`] : []),
    `F/B : ${r.fbDb.toFixed(2)} dB  後方 ${back}`,
    `Freq: ${r.frequencyMhz.toFixed(3)} MHz`,
    `Z   : ${z(r.impedance.re, r.impedance.im, 3)} Ω`,
    `SWR : ${swrText(r.swr)}（${ref.x === 0 ? ref.r : z(ref.r, ref.x)} Ω）  ${swrText(swrAgainst(r.impedance, 600))}（600 Ω）`,
    `仰角: ${r.elevationDeg.toFixed(1)}°（${ground}）`,
  ];
  if (r.elevationDeg !== r.patterns.horizontalElevationDeg) {
    const peak = Math.max(r.maxHorizontalPolDbi, r.maxVerticalPolDbi);
    lines.push(`（水平パターンの仰角 ${r.patterns.horizontalElevationDeg.toFixed(1)}°  Peak ${db(peak)} dBi）`);
  }
  return lines;
}

/** A row of the summary: label and value pairs, or one pair across the row. */
interface SummaryRow {
  pairs: [string, string][];
  /** A row of results, printed bolder than the conditions. */
  result?: boolean;
}

/** One summary row, in mm. */
const SUMMARY_ROW = 5.4;

function summaryRows(d: PrintData): SummaryRow[] {
  const { antenna: a, environment: env } = d.project;
  const r = d.result;
  const ref = env.swrReference;
  const seg = a.autoSegmentation;
  const res = d.resonance.stale ? null : d.resonance.search?.resonance;
  const rows: SummaryRow[] = r
    ? [
        {
          result: true,
          pairs: [
            ['利得', `${db(r.gainDbi)} dBi（${db(r.gainDbi - 2.15)} dBd）`],
            ['F/B 比', `${r.fbDb.toFixed(2)} dB`],
            ['打ち上げ角', `${r.elevationDeg.toFixed(1)}°`],
          ],
        },
        {
          result: true,
          pairs: [
            ['インピーダンス', `${z(r.impedance.re, r.impedance.im)} Ω`],
            // the SWR against 600 Ω stays with MMANA's lines under the vertical pattern
            ['SWR', `${swrText(r.swr)}（${ref.x === 0 ? ref.r : z(ref.r, ref.x)} Ω）`],
            ['周波数', `${r.frequencyMhz.toFixed(3)} MHz`],
          ],
        },
      ]
    : [{ pairs: [['計算結果', d.calcError ? `計算できませんでした：${d.calcError.title}（${d.calcError.detail}）` : 'まだ計算していません']] }];
  rows.push(
    {
      pairs: [
        ['大地', GROUNDS[env.ground]],
        ['地上高', env.ground === 'freeSpace' ? '—' : `${env.height} m`],
        ['線材', MATERIALS[env.wireMaterial]],
      ],
    },
    {
      pairs: [
        ['ワイヤー', `${a.wires.length} 本${r ? `・パルス ${r.pulseCount}` : ''}`],
        ['スタック', a.stack.horizontal * a.stack.vertical > 1 ? `横 ${a.stack.horizontal} × 縦 ${a.stack.vertical}` : 'なし'],
        ['自動分割', `DM1 ${seg.dm1}・DM2 ${seg.dm2}・SC ${seg.sc}・EC ${seg.ec}`],
      ],
    },
  );
  if (res) {
    const c = res.circuit;
    const circuit = c ? `・Q ${c.q.toFixed(1)}・帯域幅 ${c.bandwidthKhz.toFixed(0)} kHz・L ${c.inductanceUh.toFixed(3)} µH・C ${c.capacitancePf.toFixed(2)} pF` : '';
    rows.push({ pairs: [['共振', `${res.frequencyMhz.toFixed(4)} MHz・Z ${z(res.impedance.re, res.impedance.im)} Ω・SWR ${swrText(res.swr)}${circuit}`]] });
  }
  return rows;
}

function Summary({ d }: { d: PrintData }) {
  return (
    <div>
      <Heading note={d.resultStale && d.result ? 'アンテナを変更した後、再計算していません' : undefined}>計算結果</Heading>
      <table className="p-summary">
        <Widths shares={[11.5, 24.5, 7.5, 22, 8.5, 26]} />
        <tbody>
          {summaryRows(d).map((row, i) => (
            <tr key={i} className={row.result ? `p-result${d.resultStale ? ' p-stale' : ''}` : undefined}>
              {row.pairs.map(([k, v]) => [
                <th key={`${k}-k`}>{k}</th>,
                <td key={`${k}-v`} colSpan={row.pairs.length === 1 ? 5 : undefined}>
                  {v}
                </td>,
              ])}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// ------------------------------------------------------------- patterns ---

/** The horizontal pattern's diameter, in mm. */
const POLAR = 84;
/** What a half-circle pattern adds to half its width: the room for its labels. */
const POLAR_PAD = 7;

/** A line of MMANA's result under the vertical pattern, and the box's padding, in mm. */
const MMANA_LINE = 3.55;
const MMANA_PAD = 3;

function Patterns({ d, r }: { d: PrintData; r: CalcResult }) {
  const p = r.patterns;
  const lines = mmanaLines(r, d.project);
  // the vertical pattern (half as high as wide, plus its label margins)
  // gives way to the result lines below it, beside the horizontal pattern
  const below = lines.length * MMANA_LINE + MMANA_PAD;
  const verticalSize = Math.min(WIDTH - POLAR - 6, 2 * (POLAR - POLAR_PAD - 2 - below));
  return (
    <div>
      <Heading note={`最大利得 ${db(r.gainDbi)} dBi を 0 dB として表示・目盛 ${d.polarScale === 'mmana' ? 'MMANA' : 'dB リニア'}`}>指向性図</Heading>
      <div className="p-legend">
        {PRINT_SERIES.map((s) => (
          <span key={s.key}>
            <svg width="6mm" height="2mm" viewBox="0 0 6 2">
              <line x1={0} x2={6} y1={1} y2={1} stroke={s.color} strokeWidth={s.width} strokeDasharray={s.dash} />
            </svg>
            {s.label}
          </span>
        ))}
      </div>
      <div className={`p-patterns${d.resultStale ? ' p-stale' : ''}`}>
        <figure>
          <figcaption>水平面（仰角 {p.horizontalElevationDeg.toFixed(1)}°・上が前方）</figcaption>
          <PolarSvg kind="horizontal" cut={p.horizontal} maxDb={r.gainDbi} scale={d.polarScale} size={POLAR} />
        </figure>
        <div className="p-patterns-side">
          <figure>
            <figcaption>垂直面（X–Z 面・右が前方）</figcaption>
            <PolarSvg kind="vertical" cut={p.vertical} maxDb={r.gainDbi} scale={d.polarScale} size={verticalSize} />
          </figure>
          {/* where MMANA printed its result, under the vertical pattern */}
          <pre className="p-mmana">{lines.join('\n')}</pre>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- shape ---

/** The most height the three views take, in mm. */
const SHAPE = 72;

/** Every stacked copy, as calculated, and the views laid out for them. */
function shapeOf(d: PrintData) {
  const wires = displayWires(d.project.antenna);
  const extent = extentOf(wires)!;
  return { wires, extent, layout: layoutViews(extent, WIDTH, SHAPE) };
}

function Shape({ d }: { d: PrintData }) {
  const a = d.project.antenna;
  const { wires, extent, layout } = shapeOf(d);
  // sources and loads are marked on the first copy
  const offset = sub(wires[0].start, a.wires[0].start);
  const at = (ps: (Point3 | null)[]) => ps.flatMap((p) => (p ? [add(p, offset)] : []));
  const feeds = feedPositions(a, null);
  const sources = at(feeds.sources);
  const loads = at(feeds.loads);
  const dims = [
    `X（前後）${lengthWithUnit(extent.max.x - extent.min.x, d.units)}`,
    `Y（左右）${lengthWithUnit(extent.max.y - extent.min.y, d.units)}`,
    `Z（上下）${lengthWithUnit(extent.max.z - extent.min.z, d.units)}`,
  ];
  return (
    <div>
      <Heading note={`大きさ ${dims.join('・')}・縮尺 ${scaleText(layout.scale)}${sources.length ? '・● 給電点' : ''}${loads.length ? '・■ 負荷' : ''}`}>アンテナ形状</Heading>
      <ShapeViews wires={wires} extent={extent} layout={layout} sources={sources} loads={loads} width={WIDTH} />
    </div>
  );
}

// ------------------------------------------------------------- warnings ---

const SEVERITY: Record<Diagnostic['severity'], string> = { error: 'エラー', warning: '警告', info: '情報' };
/** Characters of a warning that fit on a line at 8 pt. */
const LINE_CHARS = 64;
const warningLines = (d: Diagnostic) => Math.ceil((d.message.length + 6) / LINE_CHARS);

function generalWarnings(d: PrintData) {
  return current(d)?.warnings.filter((w) => w.wires.length === 0) ?? [];
}

function Warnings({ d }: { d: PrintData }) {
  const general = generalWarnings(d);
  return (
    <div>
      <Heading>警告</Heading>
      {d.diagnostics.length === 0 && general.length === 0 ? (
        <div className="p-ok">✓ 問題は見つかっていません</div>
      ) : (
        <ul className="p-warnings">
          {d.diagnostics.map((w, i) => (
            <li key={i} className={`p-${w.severity}`}>
              <b>{SEVERITY[w.severity]}</b>
              {w.message}
            </li>
          ))}
          {general.map((w, i) => (
            <li key={`g${i}`} className="p-warning">
              <b>警告</b>
              {w.message}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// --------------------------------------------------------------- tables ---

interface Column<T> {
  label: string;
  /** Share of the width. */
  width: number;
  cell: (row: T, index: number) => string;
  left?: boolean;
}

/** Column widths as shares of the table's width. */
function Widths({ shares }: { shares: number[] }) {
  const total = shares.reduce((a, b) => a + b, 0);
  return (
    <colgroup>
      {shares.map((w, i) => (
        <col key={i} style={{ width: `${(w / total) * 100}%` }} />
      ))}
    </colgroup>
  );
}

function Table<T>({ title, note, columns, rows, start, end }: { title: string; note?: string; columns: Column<T>[]; rows: T[]; start: number; end: number }) {
  return (
    <div>
      <Heading note={note}>{start > 0 ? `${title}（続き）` : title}</Heading>
      <table className="p-table">
        <Widths shares={columns.map((c) => c.width)} />
        <thead>
          <tr>
            {columns.map((c) => (
              <th key={c.label} className={c.left ? 'left' : undefined}>
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.slice(start, end).map((row, k) => (
            <tr key={start + k}>
              {columns.map((c) => (
                <td key={c.label} className={c.left ? 'left' : undefined}>
                  {c.cell(row, start + k)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function elementColumns(e: ElementEdit, u: Units): Column<ElementEdit['elements'][number]>[] {
  const lu = lengthUnit(u);
  return [
    { label: 'No.', width: 4, cell: (_, i) => String(i + 1) },
    { label: '形式', width: 9, cell: (r) => KIND_LABELS[r.kind], left: true },
    { label: `位置 (${lu})`, width: 10, cell: (r) => length(r.position, u) },
    { label: `間隔 (${lu})`, width: 10, cell: (r, i) => (i === e.base ? '基準' : length(r.interval, u)) },
    { label: `横幅 (${lu})`, width: 10, cell: (r) => length(r.width, u) },
    { label: `縦幅 (${lu})`, width: 10, cell: (r) => length(r.height, u) },
    { label: `${e.elements[0]?.xWidth ? 'Ｘ幅' : '全周'} (${lu})`, width: 10, cell: (r) => length(r.perimeter, u) },
    { label: `R (${radiusUnit(u)})`, width: 8, cell: (r) => (r.radius == null ? '複合' : format(radiusToDisplay(r.radius, u), radiusDigits(u))) },
    { label: 'Seg.', width: 7, cell: (r) => (r.segmentation == null ? '複合' : formatSegmentation(r.segmentation)) },
    { label: 'ワイヤー', width: 6, cell: (r) => String(r.wireCount) },
  ];
}

type WireRow = Project['antenna']['wires'][number];

function wireColumns(u: Units): Column<WireRow>[] {
  const lu = lengthUnit(u);
  const coord = (label: string, f: (w: WireRow) => number): Column<WireRow> => ({ label: `${label}`, width: 9, cell: (w) => length(f(w), u) });
  return [
    { label: 'No.', width: 4, cell: (_, i) => String(i + 1) },
    coord(`X1 (${lu})`, (w) => w.start.x),
    coord(`Y1 (${lu})`, (w) => w.start.y),
    coord(`Z1 (${lu})`, (w) => w.start.z),
    coord(`X2 (${lu})`, (w) => w.end.x),
    coord(`Y2 (${lu})`, (w) => w.end.y),
    coord(`Z2 (${lu})`, (w) => w.end.z),
    { label: `R (${radiusUnit(u)})`, width: 7, cell: (w) => format(radiusToDisplay(w.radius, u), radiusDigits(u)) },
    { label: 'SEG', width: 5, cell: (w) => formatSegmentation(w.segmentation) },
    { label: `長さ (${lu})`, width: 9, cell: (w) => length(wireLength(w), u) },
    { label: 'メモ', width: 12, cell: (w) => w.note ?? '', left: true },
  ];
}

// ----------------------------------------------------------------- feed ---

function loadValue(l: Load): string {
  switch (l.kind) {
    case 'lc': {
      const parts = [l.inductanceUh ? `L ${l.inductanceUh} µH` : '', l.capacitancePf ? `C ${l.capacitancePf} pF` : '', l.q ? `Q ${l.q}` : ''].filter(Boolean);
      return parts.join('・') || '—';
    }
    case 'impedance':
      return `${z(l.r, l.x)} Ω`;
    case 'sDomain':
      return `S ドメイン ${l.coefficients.map((c) => `(${c.join(', ')})`).join(' ')}`;
  }
}

const LOAD_KIND: Record<Load['kind'], string> = { lc: 'LC', impedance: 'R + jX', sDomain: 'S ドメイン' };

function Feed({ d }: { d: PrintData }) {
  const a = d.project.antenna;
  const r = current(d);
  return (
    <div>
      <Heading note={r ? undefined : '計算結果がないため、インピーダンスなどは空欄です'}>給電点・負荷</Heading>
      <table className="p-table">
        <Widths shares={[7, 8, 8, 8, 22, 8, 10, 12]} />
        <thead>
          <tr>
            <th>給電点</th>
            <th className="left">パルス</th>
            <th>位相 (°)</th>
            <th>電圧 (V)</th>
            <th>インピーダンス (Ω)</th>
            <th>SWR</th>
            <th>電流 (mA)</th>
            <th>電力 (W)</th>
          </tr>
        </thead>
        <tbody>
          {a.sources.map((s, i) => {
            const sr = r?.sources[i];
            return (
              <tr key={i}>
                <td>{i + 1}</td>
                <td className="left">{s.pulse}</td>
                <td>{s.phaseDeg}</td>
                <td>{s.voltage}</td>
                <td>{sr ? z(sr.impedance.re, sr.impedance.im) : ''}</td>
                <td>{sr ? swrText(sr.swr) : ''}</td>
                <td>{sr ? (Math.hypot(sr.current.re, sr.current.im) * 1000).toFixed(2) : ''}</td>
                <td>{sr ? sr.powerW.toExponential(3) : ''}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {a.loads.length > 0 && (
        <table className="p-table" style={{ marginTop: '3mm' }}>
          <Widths shares={[7, 8, 10, 58]} />
          <thead>
            <tr>
              <th>負荷{a.loadsEnabled ? '' : '（無効）'}</th>
              <th className="left">パルス</th>
              <th className="left">種類</th>
              <th className="left">値</th>
            </tr>
          </thead>
          <tbody>
            {a.loads.map((l, i) => (
              <tr key={i}>
                <td>{i + 1}</td>
                <td className="left">{l.pulse}</td>
                <td className="left">{LOAD_KIND[l.kind]}</td>
                <td className="left">{loadValue(l)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- sweep ---

function Sweep({ d }: { d: PrintData }) {
  const data = d.sweep.data!;
  const x = data.map((p) => p.frequencyMhz);
  const marker = d.project.antenna.frequencyMhz;
  const band = (limit: number) => {
    const b = swrBandwidth(data, limit);
    return b ? `SWR < ${limit}：${b.lowMhz.toFixed(3)}〜${b.highMhz.toFixed(3)} MHz（${b.open ? '≥ ' : ''}${b.khz.toFixed(1)} kHz）` : `SWR < ${limit}：なし`;
  };
  const b15 = swrBandwidth(data, 1.5);
  const b20 = swrBandwidth(data, 2);
  const res = d.resonance.stale ? null : d.resonance.search?.resonance;
  const circuit = res?.circuit;
  const cw = (WIDTH - 4) / 2;
  const ch = 64;
  return (
    <div>
      <Heading note={d.sweep.stale ? 'アンテナを変更した後、再計算していません' : `${x[0]}〜${x.at(-1)} MHz・${data.length} 点`}>周波数特性</Heading>
      <div className="p-bandwidth">
        <div>
          {band(1.5)}　{band(2)}
        </div>
        {/* MMANA printed the fitted series circuit on its impedance chart */}
        {circuit && (
          <div>
            推測計算：fo {res!.frequencyMhz.toFixed(3)} MHz・L {circuit.inductanceUh.toFixed(3)} µH・C {circuit.capacitancePf.toFixed(2)} pF・Q {circuit.q.toFixed(1)}・B{' '}
            {circuit.bandwidthKhz.toFixed(1)} kHz
          </div>
        )}
      </div>
      <div className={`p-sweep${d.sweep.stale ? ' p-stale' : ''}`}>
        <LineSvg
          title="SWR"
          unit=""
          x={x}
          width={cw}
          height={ch}
          yMin={1}
          yMax={10}
          markerX={marker}
          levels={[1.5, 2]}
          bands={[
            ...(b20 ? [{ low: b20.lowMhz, high: b20.highMhz, className: 'p-band' }] : []),
            ...(b15 ? [{ low: b15.lowMhz, high: b15.highMhz, className: 'p-band' }] : []),
          ]}
          series={[{ label: 'SWR', values: data.map((p) => p.swr), color: 'var(--p-series-1)' }]}
        />
        <LineSvg
          title="インピーダンス"
          unit="Ω"
          x={x}
          width={cw}
          height={ch}
          markerX={marker}
          series={[
            { label: 'R', values: data.map((p) => p.impedance.re), color: 'var(--p-series-1)' },
            { label: 'jX', values: data.map((p) => p.impedance.im), color: 'var(--p-series-2)', dash: '1.6 1' },
          ]}
        />
        <LineSvg title="利得" unit="dBi" x={x} width={cw} height={ch} markerX={marker} series={[{ label: '利得', values: data.map((p) => p.gainDbi), color: 'var(--p-series-1)' }]} />
        <LineSvg title="F/B 比" unit="dB" x={x} width={cw} height={ch} markerX={marker} series={[{ label: 'F/B', values: data.map((p) => p.fbDb), color: 'var(--p-series-1)' }]} />
      </div>
    </div>
  );
}

// ----------------------------------------------------------------- memo ---

function Memo() {
  return (
    <div>
      <Heading>メモ</Heading>
      <div className="p-memo" />
    </div>
  );
}

// --------------------------------------------------------------- blocks ---

export interface PrintSection {
  block: PrintBlock;
  render: (rows?: { start: number; end: number }) => ReactNode;
}

/** Why a section has nothing to print, or null when it has. */
export function unavailable(id: SectionId, d: PrintData): string | null {
  switch (id) {
    case 'patterns':
      return d.result ? null : '計算すると印刷できます';
    case 'shape':
    case 'wires':
      return d.project.antenna.wires.length ? null : 'ワイヤーがありません';
    case 'elements':
      return d.elements?.elements.length ? null : 'エレメントとして読めるワイヤーがありません';
    case 'sweep':
      return d.sweep.data && d.sweep.data.length > 1 ? null : '周波数特性を計算すると印刷できます';
    default:
      return null;
  }
}

/** The sections chosen, in print order, each with the height it takes. */
export function printSections(d: PrintData, chosen: Record<SectionId, boolean>): PrintSection[] {
  const on = (id: SectionId) => chosen[id] && unavailable(id, d) == null;
  const out: PrintSection[] = [{ block: { key: 'title', height: 18 + commentLines(d) * 4.2 }, render: () => <Title d={d} /> }];
  if (on('summary')) out.push({ block: { key: 'summary', height: HEADING + summaryRows(d).length * SUMMARY_ROW + 1 }, render: () => <Summary d={d} /> });
  if (on('patterns')) out.push({ block: { key: 'patterns', height: HEADING + 5 + 4 + POLAR }, render: () => <Patterns d={d} r={d.result!} /> });
  if (on('shape')) out.push({ block: { key: 'shape', height: HEADING + shapeOf(d).layout.height }, render: () => <Shape d={d} /> });
  if (on('warnings')) {
    const lines = d.diagnostics.reduce((n, w) => n + warningLines(w), 0) + generalWarnings(d).length * 2;
    out.push({ block: { key: 'warnings', height: HEADING + Math.max(1, lines) * 4.2 + 2 }, render: () => <Warnings d={d} /> });
  }
  if (on('elements')) {
    const e = d.elements!;
    out.push({
      block: { key: 'elements', head: HEADING + TABLE_HEAD, rows: e.elements.length },
      render: (rows) => <Table title="エレメント表" columns={elementColumns(e, d.units)} rows={e.elements} start={rows!.start} end={rows!.end} />,
    });
  }
  if (on('wires')) {
    const wires = d.project.antenna.wires;
    out.push({
      block: { key: 'wires', head: HEADING + TABLE_HEAD, rows: wires.length },
      render: (rows) => <Table title="ワイヤー表" columns={wireColumns(d.units)} rows={wires} start={rows!.start} end={rows!.end} />,
    });
  }
  if (on('feed')) {
    const a = d.project.antenna;
    const loads = a.loads.length ? 3 + TABLE_HEAD + a.loads.length * ROW_HEIGHT : 0;
    out.push({ block: { key: 'feed', height: HEADING + TABLE_HEAD + a.sources.length * ROW_HEIGHT + loads + 1 }, render: () => <Feed d={d} /> });
  }
  if (on('sweep')) out.push({ block: { key: 'sweep', height: HEADING + 10 + 64 * 2 + 4 }, render: () => <Sweep d={d} /> });
  if (on('memo')) out.push({ block: { key: 'memo', height: HEADING + 58 }, render: () => <Memo /> });
  return out;
}
