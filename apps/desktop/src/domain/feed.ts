// Sources and loads on the pulses of the wires: where their markers go and
// where the feed tool can put them. The pulses come from the engine
// (`transport.pulseLayout`) and the names it gives them are written back as
// they are, so a placed source stays a plain MMANA pulse name (`w1b3`).
import type { Antenna, Point3, Project, PulseLayout, PulsePoint, Wire } from '../api/types';
import { normalizePulse, pulseLocation } from './geometry';
import { add, distance, samePoint, scale, sub } from './wireEdit';

export type FeedKind = 'source' | 'load';
export interface FeedRef {
  kind: FeedKind;
  index: number;
}

/** A pulse layout and the project and frequency it was computed for. */
export interface LayoutFor {
  layout: PulseLayout;
  project: Project;
  frequencyMhz: number;
}

export interface FeedPositions {
  sources: (Point3 | null)[];
  loads: (Point3 | null)[];
}

/**
 * Whether the layout's pulses are still where it says: the same wires,
 * segmentation, ground and frequency. Sources and loads may have changed.
 */
export function pulsesFit(l: LayoutFor, project: Project, frequencyMhz: number): boolean {
  const a = l.project.antenna;
  const b = project.antenna;
  return (
    l.frequencyMhz === frequencyMhz &&
    a.wires === b.wires &&
    a.autoSegmentation === b.autoSegmentation &&
    a.combinedWires === b.combinedWires &&
    l.project.environment === project.environment
  );
}

/** The pulse the layout gives this name to, if any. */
export function pulseNamed(layout: PulseLayout, name: string): PulsePoint | null {
  const n = normalizePulse(name).toLowerCase();
  for (const w of layout.wires) for (const p of w.pulses) if (p.name === n) return p;
  return null;
}

const wireOf = (name: string): number | null => {
  const m = /^w(\d+)/i.exec(normalizePulse(name));
  return m ? Number(m[1]) - 1 : null;
};

/**
 * Where the markers of the sources and loads go, in wire coordinates; null
 * when a name does not resolve. `fit` is a layout that fits `antenna`.
 * While wires are dragged (`shown`), a marker keeps its place along its wire.
 */
export function feedPositions(antenna: Antenna, fit: LayoutFor | null, shown: Wire[] = antenna.wires): FeedPositions {
  const exact = (items: { pulse: string }[], resolved: (Point3 | null)[] | undefined, same: boolean) =>
    items.map((it, i): Point3 | null => {
      if (!fit) return pulseLocation(it.pulse, antenna.wires);
      if (same) return resolved?.[i] ?? null;
      return pulseNamed(fit.layout, it.pulse)?.position ?? pulseLocation(it.pulse, antenna.wires);
    });
  const fa = fit?.project.antenna;
  const out = {
    sources: exact(antenna.sources, fit?.layout.sources, fa?.sources === antenna.sources),
    loads: exact(antenna.loads, fit?.layout.loads, fa?.loads === antenna.loads),
  };
  if (shown === antenna.wires) return out;
  const follow = (items: { pulse: string }[], at: (Point3 | null)[]) =>
    items.map((it, i) => {
      const w = wireOf(it.pulse);
      const p = at[i];
      if (w == null || !p || !antenna.wires[w] || !shown[w]) return pulseLocation(it.pulse, shown);
      const from = antenna.wires[w];
      const len = distance(from.start, from.end);
      const t = len === 0 ? 0.5 : distance(p, from.start) / len;
      return add(shown[w].start, scale(sub(shown[w].end, shown[w].start), t));
    });
  return { sources: follow(antenna.sources, out.sources), loads: follow(antenna.loads, out.loads) };
}

/**
 * The pulses a source or load can go on along a wire (0-based), from its
 * start: its own, and a joint at either end that only the wire before or
 * after it can name.
 */
export function pulsesOnWire(layout: PulseLayout, wires: Wire[], wire: number): PulsePoint[] {
  const w = wires[wire];
  const own = layout.wires.find((x) => x.wire === wire + 1)?.pulses ?? [];
  if (!w) return own;
  const seen = new Set(own.map((p) => p.pulse));
  const joints: PulsePoint[] = [];
  for (const other of layout.wires) {
    for (const p of other.pulses) {
      if (seen.has(p.pulse) || !(samePoint(p.position, w.start) || samePoint(p.position, w.end))) continue;
      seen.add(p.pulse);
      joints.push(p);
    }
  }
  if (joints.length === 0) return own;
  return [...own, ...joints].sort((a, b) => distance(a.position, w.start) - distance(b.position, w.start));
}

const fmt = (v: number) => Number(v.toFixed(4)).toString();

/** `w1b3（始点から 0.512 m・終点から 4.8 m）` */
export function describePulse(p: PulsePoint, w: Wire | undefined): string {
  if (!w) return p.name;
  const len = distance(w.start, w.end);
  const d = distance(p.position, w.start);
  return `${p.name}（始点から ${fmt(d)} m・終点から ${fmt(Math.max(0, len - d))} m）`;
}

export const feedLabel = (ref: FeedRef) => `${ref.kind === 'source' ? '給電点' : '負荷'} ${ref.index + 1}`;
