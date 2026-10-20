import { useElementSize } from '@mantine/hooks';
import { useState } from 'react';
import { niceTicks } from './ticks';

export interface Series {
  label: string;
  values: number[];
  color: string;
  dash?: string;
}

interface LineChartProps {
  title: string;
  unit: string;
  x: number[];
  series: Series[];
  /** Shared hover index (crosshair synchronised across charts). */
  hover: number | null;
  onHover: (i: number | null) => void;
  /** Clamp the y range, e.g. SWR to [1, 10]. */
  yMin?: number;
  yMax?: number;
  digits?: number;
  /** Highlight a reference x value (e.g. design frequency). */
  markerX?: number;
}

const M = { top: 8, right: 12, bottom: 22, left: 44 };

export function LineChart({ title, unit, x, series, hover, onHover, yMin, yMax, digits = 2, markerX }: LineChartProps) {
  const { ref, width, height } = useElementSize();
  // crosshair is shared; the tooltip only shows on the chart under the pointer
  const [inside, setInside] = useState(false);
  const all = series.flatMap((s) => s.values).filter(Number.isFinite);
  let lo = Math.min(...all);
  let hi = Math.max(...all);
  if (yMin != null) lo = Math.max(lo, yMin);
  if (yMax != null) hi = Math.min(hi, yMax);
  if (yMin != null && lo > yMin && lo - yMin < (hi - lo) * 0.25) lo = yMin;
  const yTicks = niceTicks(lo, hi, 4);
  const y0 = yTicks[0] ?? lo;
  const y1 = yTicks[yTicks.length - 1] ?? hi;
  const xTicks = niceTicks(x[0], x[x.length - 1], 5).filter((t) => t >= x[0] - 1e-9 && t <= x[x.length - 1] + 1e-9);

  const w = Math.max(10, width - M.left - M.right);
  const h = Math.max(10, height - M.top - M.bottom);
  const sx = (v: number) => M.left + ((v - x[0]) / (x[x.length - 1] - x[0] || 1)) * w;
  const sy = (v: number) => M.top + h - ((Math.min(Math.max(v, y0), y1) - y0) / (y1 - y0 || 1)) * h;

  const onMove = (e: React.PointerEvent<SVGSVGElement>) => {
    const px = e.clientX - e.currentTarget.getBoundingClientRect().left;
    let best = 0;
    for (let i = 1; i < x.length; i++) if (Math.abs(sx(x[i]) - px) < Math.abs(sx(x[best]) - px)) best = i;
    onHover(best);
  };

  return (
    <figure style={{ margin: 0, display: 'flex', flexDirection: 'column', minWidth: 0, minHeight: 0 }}>
      <figcaption style={{ display: 'flex', gap: 12, alignItems: 'baseline', marginBottom: 2 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>
          {title}
          {unit && <span style={{ fontWeight: 400, color: 'var(--text-muted)', marginLeft: 4 }}>({unit})</span>}
        </span>
        {series.length > 1 && (
          <span className="legend">
            {series.map((s) => (
              <span key={s.label}>
                <span className="legend-swatch" style={{ borderColor: s.color, borderTopStyle: s.dash ? 'dashed' : 'solid' }} />
                {s.label}
              </span>
            ))}
          </span>
        )}
      </figcaption>
      <div ref={ref} className="chart" style={{ flex: 1, minHeight: 90 }}>
        {width > 0 && x.length > 1 && (
          <svg
            width={width}
            height={height}
            role="img"
            aria-label={title}
            onPointerMove={onMove}
            onPointerEnter={() => setInside(true)}
            onPointerLeave={() => {
              setInside(false);
              onHover(null);
            }}
          >
            {yTicks.map((t) => (
              <g key={t}>
                <line x1={M.left} x2={M.left + w} y1={sy(t)} y2={sy(t)} stroke="var(--chart-grid)" />
                <text x={M.left - 6} y={sy(t) + 3} textAnchor="end">
                  {t}
                </text>
              </g>
            ))}
            {xTicks.map((t) => (
              <text key={t} x={sx(t)} y={M.top + h + 15} textAnchor="middle">
                {t}
              </text>
            ))}
            <line x1={M.left} x2={M.left + w} y1={M.top + h} y2={M.top + h} stroke="var(--chart-axis)" />
            {markerX != null && markerX >= x[0] && markerX <= x[x.length - 1] && (
              <line x1={sx(markerX)} x2={sx(markerX)} y1={M.top} y2={M.top + h} stroke="var(--chart-axis)" strokeDasharray="3 3" />
            )}
            {series.map((s) => (
              <polyline
                key={s.label}
                points={s.values.map((v, i) => `${sx(x[i])},${sy(v)}`).join(' ')}
                fill="none"
                stroke={s.color}
                strokeWidth={2}
                strokeDasharray={s.dash}
                strokeLinejoin="round"
              />
            ))}
            {hover != null && hover < x.length && (
              <g pointerEvents="none">
                <line x1={sx(x[hover])} x2={sx(x[hover])} y1={M.top} y2={M.top + h} stroke="var(--chart-axis)" />
                {series.map((s) => (
                  <circle key={s.label} cx={sx(x[hover])} cy={sy(s.values[hover])} r={4} fill={s.color} stroke="var(--chart-surface)" strokeWidth={2} />
                ))}
              </g>
            )}
          </svg>
        )}
        {inside && hover != null && hover < x.length && width > 0 && (
          <div className="chart-tooltip num" style={{ left: Math.min(width - 130, sx(x[hover]) + 10), top: 4 }}>
            <div style={{ fontWeight: 600 }}>{x[hover].toFixed(3)} MHz</div>
            {series.map((s) => (
              <div key={s.label} style={{ display: 'flex', justifyContent: 'space-between', gap: 10 }}>
                <span>
                  <span className="legend-swatch" style={{ borderColor: s.color, borderTopStyle: s.dash ? 'dashed' : 'solid' }} />
                  {s.label}
                </span>
                <span>{s.values[hover].toFixed(digits)}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </figure>
  );
}
