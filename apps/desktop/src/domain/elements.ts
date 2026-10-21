// Element editor (MMANA's エレメント編集): labels, display units, the CSV
// export and find & replace. The editing itself runs in mmana-core
// (`elements.rs`); the UI sends the list of edits and shows the result.
import type { ElementKind, ElementOp, ElementRow, Segmentation } from '../api/types';
import { formatSegmentation } from './segmentation';

export const KIND_LABELS: Record<ElementKind, string> = {
  unknown: '不明',
  verticalDipole: '垂直DP',
  verticalSquare: '垂直□',
  verticalDiamond: '垂直◇',
  verticalTriangle: '垂直△',
  verticalInvertedTriangle: '垂直▽',
  verticalOctagon: '垂直○',
  verticalLoop: '垂直ループ',
  horizontalDipole: '水平DP',
  horizontalSquare: '水平□',
  horizontalDiamond: '水平◇',
  horizontalTriangle: '水平△',
  horizontalInvertedTriangle: '水平▽',
  horizontalLoop: '水平ループ',
  horizontalOther: '水平系',
  verticalOther: '垂直系',
  verticalDoubleSquare: '垂直双□',
  verticalDoubleTriangle: '垂直双▽',
};

/** The forms an element can be rebuilt as, grouped like MMANA's menu. */
export const KIND_MENU: ElementKind[][] = [
  ['horizontalDipole', 'verticalDipole'],
  [
    'verticalSquare',
    'verticalDiamond',
    'verticalInvertedTriangle',
    'verticalTriangle',
    'verticalOctagon',
    'verticalDoubleSquare',
    'verticalDoubleTriangle',
  ],
  ['horizontalSquare', 'horizontalDiamond', 'horizontalInvertedTriangle', 'horizontalTriangle'],
];

/**
 * How lengths are shown: metres or millimetres (通常単位をmmで表示), or
 * wavelengths at the design frequency (波長表記), which takes precedence.
 */
export interface Units {
  wavelengths: boolean;
  millimetres: boolean;
  lambda: number;
}

/** `GetRmdVal` / `SetRmdVal` */
export const lengthToDisplay = (m: number, u: Units) => (u.wavelengths ? m / u.lambda : u.millimetres ? m * 1000 : m);
export const lengthFromDisplay = (d: number, u: Units) => (u.wavelengths ? d * u.lambda : u.millimetres ? d / 1000 : d);
/** The radius in mm, or in wavelengths (`GetRmdValD` / `SetRmdValD`); millimetres do not change it. */
export const radiusToDisplay = (m: number, u: Units) => (u.wavelengths ? (m * 1000 * 0.001) / u.lambda : m * 1000);
export const radiusFromDisplay = (d: number, u: Units) => (u.wavelengths ? (d * 1000 * u.lambda) / 1000 : d / 1000);

/** `GetLenUnitText` */
export const lengthUnit = (u: Units) => (u.wavelengths ? 'λ' : u.millimetres ? 'mm' : 'm');
export const radiusUnit = (u: Units) => (u.wavelengths ? 'λ' : 'mm');
/** The wavelength in the unit lengths are shown in, for `R` in expressions. */
export const lengthWavelength = (u: Units) => (u.wavelengths ? 1 : u.millimetres ? u.lambda * 1000 : u.lambda);
/** The wavelength in the unit the radius is shown in. */
export const radiusWavelength = (u: Units) => (u.wavelengths ? 1 : u.lambda * 1000);
/** Decimal places the tables show (about 0.1 mm in metres). */
export const lengthDigits = (u: Units) => (u.wavelengths ? 5 : u.millimetres ? 3 : 4);
export const radiusDigits = (u: Units) => (u.wavelengths ? 7 : 3);

/** `StrDbl`: six decimals with trailing zeros removed (one digit is kept after the point). */
export function strDbl(d: number): string {
  let s = d.toFixed(6);
  while (s.endsWith('0') && !s.endsWith('.0')) s = s.slice(0, -1);
  return s;
}

/** Numeric columns of the element table. */
export type ElementColumn = 'interval' | 'position' | 'width' | 'height' | 'perimeter' | 'radius' | 'segmentation';

export const COLUMN_LABELS: Record<ElementColumn, string> = {
  interval: '間隔',
  position: '位置',
  width: '横幅',
  height: '縦幅',
  perimeter: '全周 / Ｘ幅',
  radius: 'R',
  segmentation: 'Seg.',
};

/**
 * The value a cell shows, in display units; null where it shows text instead
 * (the base element's interval, 複合 radius or segmentation).
 */
export function cellValue(rows: ElementRow[], base: number, row: number, col: ElementColumn, u: Units): number | null {
  const r = rows[row];
  switch (col) {
    case 'interval':
      return row === base ? null : lengthToDisplay(r.interval, u);
    case 'position':
      return lengthToDisplay(r.position, u);
    case 'width':
    case 'height':
    case 'perimeter':
      return lengthToDisplay(r[col], u);
    case 'radius':
      return r.radius == null ? null : radiusToDisplay(r.radius, u);
    case 'segmentation':
      return null;
  }
}

/** The edit a value typed into a cell makes (display units in, metres out). */
export function cellOp(row: number, col: ElementColumn, value: number, u: Units, scaleAll: boolean): ElementOp {
  switch (col) {
    case 'interval':
      return { op: 'setInterval', row, value: lengthFromDisplay(value, u) };
    case 'position':
      return { op: 'setPosition', row, value: lengthFromDisplay(value, u) };
    case 'width':
      return { op: 'setWidth', row, value: lengthFromDisplay(value, u), scaleAll };
    case 'height':
      return { op: 'setHeight', row, value: lengthFromDisplay(value, u), scaleAll };
    case 'perimeter':
      return { op: 'setPerimeter', row, value: lengthFromDisplay(value, u), scaleAll };
    case 'radius':
      return { op: 'setRadius', row, value: radiusFromDisplay(value, u) };
    case 'segmentation':
      throw new Error('segmentation is not numeric');
  }
}

/**
 * The dialog's input checks, on the value as typed in the unit the table
 * shows (mmana-core only gets metres, so these are checked here).
 */
export function acceptsValue(col: ElementColumn, v: number): boolean {
  if (col === 'interval' || col === 'position') return v >= 0;
  if (col === 'width' || col === 'height' || col === 'perimeter') return v >= 0.001;
  return true;
}

/**
 * Find & replace (検索と置換): sets `replacement` in every row of `col` whose
 * value, as the table shows it with six decimals, equals `search`. Like typing
 * it, a replacement the cell would not accept changes nothing.
 */
export function replaceOps(
  rows: ElementRow[],
  base: number,
  col: ElementColumn,
  search: number | Segmentation,
  replacement: number | Segmentation,
  u: Units,
  scaleAll: boolean,
): ElementOp[] {
  const ops: ElementOp[] = [];
  rows.forEach((r, row) => {
    if (col === 'segmentation') {
      if (typeof search === 'number' || typeof replacement === 'number' || r.segmentation == null) return;
      if (formatSegmentation(r.segmentation) === formatSegmentation(search)) {
        ops.push({ op: 'setSegmentation', row, segmentation: replacement });
      }
      return;
    }
    if (typeof search !== 'number' || typeof replacement !== 'number') return;
    const value = Number(strDbl(replacement));
    if (!acceptsValue(col, value)) return;
    const v = cellValue(rows, base, row, col, u);
    if (v != null && Number(strDbl(v)) === search) ops.push(cellOp(row, col, value, u, scaleAll));
  });
  return ops;
}

const csvField = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);

/** The table as CSV (CSVファイルの作成), with both positions and intervals. */
export function elementsCsv(rows: ElementRow[], base: number, u: Units): string {
  const lu = lengthUnit(u);
  const head = [
    'No.',
    '形式',
    `位置(${lu})`,
    `間隔(${lu})`,
    `横幅(${lu})`,
    `縦幅(${lu})`,
    `${rows[0]?.xWidth ? 'Ｘ幅' : '全周'}(${lu})`,
    `R(${radiusUnit(u)})`,
    'Seg.',
    'ワイヤ数',
  ];
  const lines = rows.map((r, i) => [
    String(i + 1),
    KIND_LABELS[r.kind],
    strDbl(lengthToDisplay(r.position, u)),
    i === base ? '基準位置' : strDbl(lengthToDisplay(r.interval, u)),
    strDbl(lengthToDisplay(r.width, u)),
    strDbl(lengthToDisplay(r.height, u)),
    strDbl(lengthToDisplay(r.perimeter, u)),
    r.radius == null ? '複合' : strDbl(radiusToDisplay(r.radius, u)),
    r.segmentation == null ? '複合' : formatSegmentation(r.segmentation),
    String(r.wireCount),
  ]);
  return [head, ...lines].map((l) => l.map(csvField).join(',')).join('\r\n') + '\r\n';
}

export interface Projection {
  /** Rotation about Z, degrees (0 = looking from +X). */
  azimuth: number;
  /** Height of the viewpoint, degrees (90 = from above). */
  elevation: number;
}

/** Orthographic view: screen x to the right, screen y up. */
export function project(p: { x: number; y: number; z: number }, v: Projection): [number, number] {
  const a = (v.azimuth * Math.PI) / 180;
  const e = (v.elevation * Math.PI) / 180;
  const x = -p.x * Math.sin(a) + p.y * Math.cos(a);
  const y = -p.x * Math.sin(e) * Math.cos(a) - p.y * Math.sin(e) * Math.sin(a) + p.z * Math.cos(e);
  return [x, y];
}
