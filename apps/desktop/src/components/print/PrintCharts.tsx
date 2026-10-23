// Static figures for the printed sheets, drawn in millimetres (one SVG unit
// is 1 mm) so they print at the size laid out. They share the scales of the
// screen's plots but none of their pointer handling.
import type { PatternCut, Point3 } from '../../api/types';
import { RINGS_DB, radiusFor, sampleAngle, type CutKind, type PolarScale } from '../../charts/polar';
import { niceTicks } from '../../charts/ticks';
import type { Extent } from '../../domain/print';

/** 7 pt in mm, the size of the figures' labels. */
const LABEL = 2.5;

export const PRINT_SERIES = [
  { key: 'total', label: '合成', color: 'var(--p-series-1)', dash: undefined, width: 0.5 },
  { key: 'verticalPol', label: '垂直偏波', color: 'var(--p-series-2)', dash: '1.6 1', width: 0.35 },
  { key: 'horizontalPol', label: '水平偏波', color: 'var(--p-series-3)', dash: '0.6 0.9', width: 0.35 },
] as const;

interface PolarProps {
  kind: CutKind;
  cut: PatternCut;
  maxDb: number;
  scale: PolarScale;
  /** Width in mm; a vertical cut is half as high. */
  size: number;
}

export function PolarSvg({ kind, cut, maxDb, scale, size }: PolarProps) {
  const half = kind === 'vertical';
  // room for the angle labels outside the rim
  const pad = 7;
  const R = size / 2 - pad;
  const width = size;
  const height = half ? R + pad * 2 : size;
  const cx = width / 2;
  const cy = half ? R + pad : size / 2;
  const pt = (i: number, db: number) => {
    const a = sampleAngle(kind, i, cut.stepDeg);
    const r = R * radiusFor(db, maxDb, scale);
    return `${(cx + r * Math.cos(a)).toFixed(2)},${(cy - r * Math.sin(a)).toFixed(2)}`;
  };
  const path = (values: number[]) => values.map((v, i) => `${i ? 'L' : 'M'}${pt(i, v)}`).join('') + (half ? '' : 'Z');
  const spokes = half ? [0, 30, 60, 90, 120, 150, 180] : Array.from({ length: 12 }, (_, i) => i * 30);

  return (
    <svg width={`${width}mm`} height={`${height}mm`} viewBox={`0 0 ${width} ${height}`} className="p-figure">
      {RINGS_DB[scale].map((db) => {
        const r = R * radiusFor(maxDb + db, maxDb, scale);
        return (
          <g key={db}>
            {half ? (
              <path d={`M${cx - r},${cy}A${r},${r} 0 0 1 ${cx + r},${cy}`} className="p-grid" />
            ) : (
              <circle cx={cx} cy={cy} r={r} className="p-grid" />
            )}
            <text x={cx + 0.8} y={cy - r + LABEL} fontSize={LABEL * 0.85}>
              {db === 0 ? '0 dB' : db}
            </text>
          </g>
        );
      })}
      {spokes.map((deg) => {
        const a = sampleAngle(kind, deg / cut.stepDeg, cut.stepDeg);
        const label = half ? `${deg <= 90 ? deg : 180 - deg}°` : `${deg}°`;
        return (
          <g key={deg}>
            <line x1={cx} y1={cy} x2={cx + R * Math.cos(a)} y2={cy - R * Math.sin(a)} className="p-grid" />
            <text x={cx + (R + 2.6) * Math.cos(a)} y={cy - (R + 2.6) * Math.sin(a) + LABEL * 0.35} fontSize={LABEL} textAnchor="middle">
              {label}
            </text>
          </g>
        );
      })}
      {half && <line x1={cx - R} y1={cy} x2={cx + R} y2={cy} className="p-axis" />}
      {[...PRINT_SERIES].reverse().map((s) => (
        <path key={s.key} d={path(cut[s.key])} fill="none" stroke={s.color} strokeWidth={s.width} strokeDasharray={s.dash} strokeLinejoin="round" />
      ))}
    </svg>
  );
}

export interface LineSeries {
  label: string;
  values: number[];
  color: string;
  dash?: string;
}

interface LineProps {
  title: string;
  unit: string;
  x: number[];
  series: LineSeries[];
  width: number;
  height: number;
  yMin?: number;
  yMax?: number;
  /** A dashed line at the design frequency. */
  markerX?: number;
  /** Shaded frequency bands, e.g. where the SWR is under 2. */
  bands?: { low: number; high: number; className: string }[];
  /** Dotted horizontal lines, e.g. SWR 1.5 and 2. */
  levels?: number[];
}

const M = { top: 6, right: 3, bottom: 7, left: 10 };

export function LineSvg({ title, unit, x, series, width, height, yMin, yMax, markerX, bands = [], levels = [] }: LineProps) {
  const all = series.flatMap((s) => s.values).filter(Number.isFinite);
  let lo = Math.min(...all);
  let hi = Math.max(...all);
  if (yMin != null) lo = Math.max(lo, yMin);
  if (yMax != null) hi = Math.min(hi, yMax);
  if (yMin != null && lo > yMin && lo - yMin < (hi - lo) * 0.25) lo = yMin;
  const yTicks = niceTicks(lo, hi, 4);
  const y0 = yTicks[0] ?? lo;
  const y1 = yTicks.at(-1) ?? hi;
  const xTicks = niceTicks(x[0], x.at(-1)!, 5).filter((t) => t >= x[0] - 1e-9 && t <= x.at(-1)! + 1e-9);
  const w = width - M.left - M.right;
  const h = height - M.top - M.bottom;
  const sx = (v: number) => M.left + ((v - x[0]) / (x.at(-1)! - x[0] || 1)) * w;
  const sy = (v: number) => M.top + h - ((Math.min(Math.max(v, y0), y1) - y0) / (y1 - y0 || 1)) * h;

  return (
    <svg width={`${width}mm`} height={`${height}mm`} viewBox={`0 0 ${width} ${height}`} className="p-figure">
      <text x={M.left} y={3.4} fontSize={2.9} className="p-title">
        {title}
        {unit && <tspan className="p-muted">{` (${unit})`}</tspan>}
      </text>
      {series.length > 1 && (
        <g>
          {series.map((s, i) => {
            const lx = width - M.right - (series.length - i) * 16;
            return (
              <g key={s.label}>
                <line x1={lx} x2={lx + 5} y1={2.4} y2={2.4} stroke={s.color} strokeWidth={0.45} strokeDasharray={s.dash} />
                <text x={lx + 6} y={3.2} fontSize={LABEL}>
                  {s.label}
                </text>
              </g>
            );
          })}
        </g>
      )}
      {bands.map((b, i) => (
        <rect key={i} x={sx(b.low)} y={M.top} width={Math.max(0, sx(b.high) - sx(b.low))} height={h} className={b.className} />
      ))}
      {yTicks.map((t) => (
        <g key={t}>
          <line x1={M.left} x2={M.left + w} y1={sy(t)} y2={sy(t)} className="p-grid" />
          <text x={M.left - 1.2} y={sy(t) + LABEL * 0.35} fontSize={LABEL} textAnchor="end">
            {t}
          </text>
        </g>
      ))}
      {xTicks.map((t) => (
        <text key={t} x={sx(t)} y={M.top + h + 3.6} fontSize={LABEL} textAnchor="middle">
          {t}
        </text>
      ))}
      <text x={M.left + w} y={height - 0.4} fontSize={LABEL * 0.9} textAnchor="end" className="p-muted">
        MHz
      </text>
      {levels
        .filter((v) => v > y0 && v < y1)
        .map((v) => (
          <line key={v} x1={M.left} x2={M.left + w} y1={sy(v)} y2={sy(v)} className="p-axis" strokeDasharray="0.6 0.8" />
        ))}
      <line x1={M.left} x2={M.left + w} y1={M.top + h} y2={M.top + h} className="p-axis" />
      {markerX != null && markerX >= x[0] && markerX <= x.at(-1)! && (
        <line x1={sx(markerX)} x2={sx(markerX)} y1={M.top} y2={M.top + h} className="p-axis" strokeDasharray="1 1" />
      )}
      {series.map((s) => (
        <polyline
          key={s.label}
          points={s.values.map((v, i) => `${sx(x[i]).toFixed(2)},${sy(v).toFixed(2)}`).join(' ')}
          fill="none"
          stroke={s.color}
          strokeWidth={0.45}
          strokeDasharray={s.dash}
          strokeLinejoin="round"
        />
      ))}
    </svg>
  );
}

type Axis = 'x' | 'y' | 'z';

/** One of the three views: the axis running right and the one running up, each possibly reversed. */
interface View {
  title: string;
  right: { axis: Axis; sign: 1 | -1 };
  up: { axis: Axis; sign: 1 | -1 };
}

// Third-angle projection, so a beam reads as built: the top view (forward
// up) stands over the view from behind (sharing Y), and the side view
// (forward right) stands beside it (sharing Z). Each is a true view, not
// a mirror image.
const TOP: View = { title: '上から（↑ +X 前方・← +Y）', right: { axis: 'y', sign: -1 }, up: { axis: 'x', sign: 1 } };
const REAR: View = { title: '後ろから（↑ +Z・← +Y）', right: { axis: 'y', sign: -1 }, up: { axis: 'z', sign: 1 } };
const SIDE: View = { title: '横から（→ +X 前方・↑ +Z）', right: { axis: 'x', sign: 1 }, up: { axis: 'z', sign: 1 } };

/** Size a view keeps along an axis the antenna has no extent on. */
const FLAT = 5;
const GAP = 8;
const CAPTION = 4;
/** The width the longest caption takes. */
const CAPTION_WIDTH = 34;

interface Area {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ViewLayout {
  /** mm on paper per metre. */
  scale: number;
  top: Area;
  rear: Area;
  side: Area;
  /** The height the views take, in mm. */
  height: number;
}

/**
 * Places the three views at the largest scale that fits `width` and
 * `maxHeight`, leaving each view room for its caption.
 */
export function layoutViews(extent: Extent, width: number, maxHeight: number): ViewLayout {
  const span = { x: extent.max.x - extent.min.x, y: extent.max.y - extent.min.y, z: extent.max.z - extent.min.z };
  const at = (s: number): ViewLayout => {
    const size = (k: Axis) => Math.max(span[k] * s, FLAT);
    const top: Area = { x: 2, y: CAPTION, w: size('y'), h: size('x') };
    const rear: Area = { x: 2, y: top.y + top.h + GAP + CAPTION, w: size('y'), h: size('z') };
    const side: Area = { x: rear.x + Math.max(rear.w, CAPTION_WIDTH) + GAP, y: rear.y, w: size('x'), h: size('z') };
    return { scale: s, top, rear, side, height: rear.y + rear.h + 1 };
  };
  const fits = (l: ViewLayout) => l.side.x + Math.max(l.side.w, CAPTION_WIDTH) <= width && l.height <= maxHeight;
  // the size of a view grows with the scale, so the largest that fits is found by halving
  let lo = 0;
  let hi = width / Math.max(span.x, span.y, span.z, 1e-9);
  for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    if (fits(at(mid))) lo = mid;
    else hi = mid;
  }
  return at(lo);
}

interface ShapeProps {
  wires: { start: Point3; end: Point3 }[];
  extent: Extent;
  layout: ViewLayout;
  /** Where the sources and loads are, in metres. */
  sources: Point3[];
  loads: Point3[];
  width: number;
}

/** The three views at one scale. */
export function ShapeViews({ wires, extent, layout, sources, loads, width }: ShapeProps) {
  const s = layout.scale;
  const span = { x: extent.max.x - extent.min.x, y: extent.max.y - extent.min.y, z: extent.max.z - extent.min.z };

  const draw = (v: View, area: Area) => {
    // a reversed axis runs from its maximum; a flat one sits in the middle
    const along = (p: Point3, d: View['right'], room: number) => {
      const from = d.sign > 0 ? p[d.axis] - extent.min[d.axis] : extent.max[d.axis] - p[d.axis];
      return (room - span[d.axis] * s) / 2 + from * s;
    };
    const px = (p: Point3) => area.x + along(p, v.right, area.w);
    const py = (p: Point3) => area.y + area.h - along(p, v.up, area.h);
    return (
      <g key={v.title}>
        <text x={area.x} y={area.y - 1.4} fontSize={LABEL} className="p-muted">
          {v.title}
        </text>
        {wires.map((w, i) => (
          <line key={i} x1={px(w.start)} y1={py(w.start)} x2={px(w.end)} y2={py(w.end)} className="p-wire" />
        ))}
        {loads.map((l, i) => (
          <rect key={`l${i}`} x={px(l) - 0.8} y={py(l) - 0.8} width={1.6} height={1.6} className="p-load" />
        ))}
        {sources.map((f, i) => (
          <circle key={`s${i}`} cx={px(f)} cy={py(f)} r={0.9} className="p-feed" />
        ))}
      </g>
    );
  };

  return (
    <svg width={`${width}mm`} height={`${layout.height}mm`} viewBox={`0 0 ${width} ${layout.height}`} className="p-figure">
      {draw(TOP, layout.top)}
      {draw(REAR, layout.rear)}
      {draw(SIDE, layout.side)}
    </svg>
  );
}

/** The scale of a drawing as 1 : N, N to three figures. */
export const scaleText = (mmPerMetre: number) => `1 : ${Number((1000 / mmPerMetre).toPrecision(3))}`;
