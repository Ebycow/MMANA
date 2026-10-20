import { useElementSize } from '@mantine/hooks';
import { useState } from 'react';
import type { PatternCut } from '../api/types';
import { RINGS_DB, radiusFor, sampleAngle, sampleAt, sampleLabel, type CutKind } from './polar';

export const SERIES = [
  { key: 'total', label: '合成', color: 'var(--series-1)', dash: undefined, width: 2 },
  { key: 'verticalPol', label: '垂直偏波', color: 'var(--series-2)', dash: '5 3', width: 1.5 },
  { key: 'horizontalPol', label: '水平偏波', color: 'var(--series-3)', dash: '2 3', width: 1.5 },
] as const;

interface PolarPlotProps {
  kind: CutKind;
  cut: PatternCut;
  /** Overall maximum gain the plot is normalised to (dBi). */
  maxDb: number;
  title: string;
  subtitle: string;
}

export function PolarPlot({ kind, cut, maxDb, title, subtitle }: PolarPlotProps) {
  const { ref, width, height } = useElementSize();
  const [hover, setHover] = useState<number | null>(null);
  const pad = 24;
  const half = kind === 'vertical';
  // vertical cut is a half circle: width = 2R, height = R
  const R = Math.max(10, Math.min((width - pad * 2) / 2, half ? height - pad * 2 : (height - pad * 2) / 2));
  const cx = width / 2;
  const cy = half ? pad + R + 6 : height / 2;
  const n = cut.total.length;

  const pt = (i: number, db: number) => {
    const a = sampleAngle(kind, i, cut.stepDeg);
    const r = R * radiusFor(db, maxDb);
    return [cx + r * Math.cos(a), cy - r * Math.sin(a)] as const;
  };
  const path = (values: number[]) => {
    const d = values.map((v, i) => `${i ? 'L' : 'M'}${pt(i, v).join(',')}`).join('');
    return kind === 'horizontal' ? `${d}Z` : d;
  };
  const spokes = kind === 'horizontal' ? Array.from({ length: 12 }, (_, i) => i * 30) : [0, 30, 60, 90, 120, 150, 180];

  const onMove = (e: React.PointerEvent<SVGSVGElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    setHover(sampleAt(kind, e.clientX - box.left - cx, e.clientY - box.top - cy, cut.stepDeg, n));
  };

  const tip = hover != null ? pt(hover, cut.total[hover]) : null;

  return (
    <figure style={{ margin: 0, display: 'flex', flexDirection: 'column', minWidth: 0, flex: 1 }}>
      <figcaption style={{ display: 'flex', alignItems: 'baseline', gap: 8, marginBottom: 2 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>{title}</span>
        <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{subtitle}</span>
      </figcaption>
      <div ref={ref} className="chart" style={{ flex: 1, minHeight: 160 }}>
        {width > 0 && (
          <svg
            width={width}
            height={height}
            role="img"
            aria-label={`${title}の指向性`}
            onPointerMove={onMove}
            onPointerLeave={() => setHover(null)}
          >
            {/* recessive grid: dB rings and angle spokes */}
            {RINGS_DB.map((db) => {
              const r = R * radiusFor(maxDb + db, maxDb);
              return (
                <g key={db}>
                  {half ? (
                    <path d={`M${cx - r},${cy}A${r},${r} 0 0 1 ${cx + r},${cy}`} fill="none" stroke="var(--chart-grid)" />
                  ) : (
                    <circle cx={cx} cy={cy} r={r} fill="none" stroke="var(--chart-grid)" />
                  )}
                  <text x={cx + 3} y={cy - r + 11}>
                    {db === 0 ? '0 dB' : db}
                  </text>
                </g>
              );
            })}
            {spokes.map((deg) => {
              const a = sampleAngle(kind, deg / cut.stepDeg, cut.stepDeg);
              const x = cx + R * Math.cos(a);
              const y = cy - R * Math.sin(a);
              const lx = cx + (R + 12) * Math.cos(a);
              const ly = cy - (R + 12) * Math.sin(a);
              const label = kind === 'horizontal' ? `${deg}°` : `${deg <= 90 ? deg : 180 - deg}°`;
              return (
                <g key={deg}>
                  <line x1={cx} y1={cy} x2={x} y2={y} stroke="var(--chart-grid)" />
                  <text x={lx} y={ly + 3} textAnchor="middle">
                    {label}
                  </text>
                </g>
              );
            })}
            {half && <line x1={cx - R} y1={cy} x2={cx + R} y2={cy} stroke="var(--chart-axis)" />}

            {[...SERIES].reverse().map((s) => (
              <path
                key={s.key}
                d={path(cut[s.key])}
                fill="none"
                stroke={s.color}
                strokeWidth={s.width}
                strokeDasharray={s.dash}
                strokeLinejoin="round"
              />
            ))}

            {hover != null && tip && (
              <g pointerEvents="none">
                <line x1={cx} y1={cy} x2={tip[0]} y2={tip[1]} stroke="var(--chart-axis)" strokeDasharray="2 2" />
                <circle cx={tip[0]} cy={tip[1]} r={4} fill="var(--series-1)" stroke="var(--chart-surface)" strokeWidth={2} />
              </g>
            )}
          </svg>
        )}
        {hover != null && tip && (
          <div
            className="chart-tooltip"
            style={{ left: Math.min(width - 150, tip[0] + 12), top: Math.max(0, tip[1] - 64) }}
            role="status"
          >
            <div style={{ fontWeight: 600, marginBottom: 2 }}>{sampleLabel(kind, hover, cut.stepDeg)}</div>
            {SERIES.map((s) => (
              <div key={s.key} className="num" style={{ display: 'flex', gap: 8, justifyContent: 'space-between' }}>
                <span>
                  <span className="legend-swatch" style={{ borderColor: s.color, borderTopStyle: s.dash ? 'dashed' : 'solid' }} />
                  {s.label}
                </span>
                <span>{formatDb(cut[s.key][hover])}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </figure>
  );
}

export const formatDb = (v: number) => (v <= -998 ? '—' : `${v.toFixed(2)} dBi`);

export function PolarLegend() {
  return (
    <div className="legend">
      {SERIES.map((s) => (
        <span key={s.key}>
          <span className="legend-swatch" style={{ borderColor: s.color, borderTopStyle: s.dash ? 'dashed' : 'solid' }} />
          {s.label}
        </span>
      ))}
    </div>
  );
}
