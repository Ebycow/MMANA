// The element editor's drawing of the edited antenna (the dialog's アンテナ形状
// page): an orthographic view that can be turned, element numbers, the
// selected element highlighted. Clicking a wire selects its element.
import { useElementSize } from '@mantine/hooks';
import { useMemo, useState } from 'react';
import type { Source, Wire } from '../api/types';
import { project, type Projection } from '../domain/elements';
import { pulseLocation } from '../domain/geometry';

interface ElementPreviewProps {
  wires: Wire[];
  /** Element (row) of each wire. */
  wireElements: number[];
  sources: Source[];
  base: number;
  selected: number | null;
  view: Projection;
  onSelect: (row: number) => void;
}

const PAD = 28;
const AXES = [
  { label: 'X', p: { x: 1, y: 0, z: 0 }, color: '#e34948' },
  { label: 'Y', p: { x: 0, y: 1, z: 0 }, color: '#1baf7a' },
  { label: 'Z', p: { x: 0, y: 0, z: 1 }, color: '#2a78d6' },
];

export function ElementPreview({ wires, wireElements, sources, base, selected, view, onSelect }: ElementPreviewProps) {
  const { ref, width, height } = useElementSize();
  const [hover, setHover] = useState<number | null>(null);

  const drawing = useMemo(() => {
    const segs = wires.map((w, i) => ({ a: project(w.start, view), b: project(w.end, view), row: wireElements[i] ?? -1 }));
    const feeds = sources
      .map((s) => pulseLocation(s.pulse, wires))
      .filter((p) => p != null)
      .map((p) => project(p, view));
    const lo = [Infinity, Infinity];
    const hi = [-Infinity, -Infinity];
    for (const s of segs) {
      for (const p of [s.a, s.b]) {
        lo[0] = Math.min(lo[0], p[0]);
        lo[1] = Math.min(lo[1], p[1]);
        hi[0] = Math.max(hi[0], p[0]);
        hi[1] = Math.max(hi[1], p[1]);
      }
    }
    // element numbers above each element
    const labels = new Map<number, { x: number; y: number; n: number }>();
    for (const s of segs) {
      for (const p of [s.a, s.b]) {
        const l = labels.get(s.row) ?? { x: 0, y: -Infinity, n: 0 };
        labels.set(s.row, { x: l.x + p[0], y: Math.max(l.y, p[1]), n: l.n + 1 });
      }
    }
    return { segs, feeds, lo, hi, labels };
  }, [wires, wireElements, sources, view]);

  const { segs, feeds, lo, hi, labels } = drawing;
  const span = Math.max(hi[0] - lo[0], hi[1] - lo[1]);
  const scale =
    segs.length === 0 || width === 0
      ? 0
      : Math.min((width - 2 * PAD) / Math.max(hi[0] - lo[0], span * 1e-3), (height - 2 * PAD) / Math.max(hi[1] - lo[1], span * 1e-3));
  const cx = (lo[0] + hi[0]) / 2;
  const cy = (lo[1] + hi[1]) / 2;
  const px = (p: [number, number]) => [width / 2 + (p[0] - cx) * scale, height / 2 - (p[1] - cy) * scale] as const;

  return (
    <div ref={ref} style={{ position: 'absolute', inset: 0 }}>
      {width > 0 && (
        <svg width={width} height={height} role="img" aria-label="アンテナ形状" style={{ display: 'block' }}>
          {segs.map((s, i) => {
            const [x1, y1] = px(s.a);
            const [x2, y2] = px(s.b);
            const sel = s.row === selected;
            return (
              <g key={i}>
                <line
                  x1={x1}
                  y1={y1}
                  x2={x2}
                  y2={y2}
                  stroke={sel ? 'var(--wire-selected)' : 'var(--wire)'}
                  strokeWidth={sel ? 3 : hover === s.row ? 2.4 : 1.4}
                  strokeLinecap="round"
                />
                <line
                  x1={x1}
                  y1={y1}
                  x2={x2}
                  y2={y2}
                  stroke="transparent"
                  strokeWidth={10}
                  style={{ cursor: 'pointer' }}
                  onPointerEnter={() => setHover(s.row)}
                  onPointerLeave={() => setHover(null)}
                  onClick={() => onSelect(s.row)}
                />
              </g>
            );
          })}
          {feeds.map((p, i) => {
            const [x, y] = px(p);
            return <circle key={i} cx={x} cy={y} r={4} fill="#eb6834" pointerEvents="none" />;
          })}
          {[...labels].map(([row, l]) => {
            const [x, y] = px([l.x / l.n, l.y]);
            return (
              <text
                key={row}
                x={x}
                y={y - 6}
                textAnchor="middle"
                fontSize={10}
                fill={row === selected ? 'var(--wire-selected)' : 'var(--text-muted)'}
                fontWeight={row === base ? 700 : 400}
                pointerEvents="none"
              >
                {row + 1}
              </text>
            );
          })}
          <g transform={`translate(${PAD}, ${height - PAD})`} pointerEvents="none">
            {AXES.map((a) => {
              const [x, y] = project(a.p, view);
              return (
                <g key={a.label}>
                  <line x1={0} y1={0} x2={x * 18} y2={-y * 18} stroke={a.color} strokeWidth={1.5} />
                  <text x={x * 24} y={-y * 24 + 3} fontSize={9} textAnchor="middle" fill={a.color}>
                    {a.label}
                  </text>
                </g>
              );
            })}
          </g>
        </svg>
      )}
    </div>
  );
}
