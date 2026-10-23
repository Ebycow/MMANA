// Direct editing inside the 3D canvas: picking, selection, dragging wires and
// their ends, box selection, drawing wires and loops, the move / rotate gizmo,
// moving sources and loads between pulses, and the overlays that go with them.
//
// Picking is done in screen space, in a capture listener on the canvas
// container that runs before OrbitControls and the gizmo: when the pointer
// goes down on a wire, an end or (with the box-selection key, Shift by
// default) empty space, the event stops there; otherwise it reaches the
// camera controls as before.
import { Grid, Html, Line, TransformControls } from '@react-three/drei';
import { useThree } from '@react-three/fiber';
import { useEffect, useLayoutEffect, useMemo, useRef, type RefObject } from 'react';
import { Group, Plane as ThreePlane, Raycaster, Vector2, Vector3, type Camera } from 'three';
import type { TransformControls as TransformControlsImpl } from 'three-stdlib';
import type { Point3, PulsePoint, Wire } from '../api/types';
import { describePulse, feedLabel, pulsesOnWire, type FeedPositions, type FeedRef, type LayoutFor } from '../domain/feed';
import type { DisplayWire } from '../domain/geometry';
import { isHeld } from '../domain/shortcuts';
import { along } from '../domain/junctions';
import {
  PLANE_NORMAL,
  add,
  distToSegment2,
  distance,
  dominantAxis,
  moveEndpoint,
  norm,
  polygonPoints,
  polylineWires,
  rectanglePoints,
  samePoint,
  scale,
  segmentHitsRect,
  selectionCenter,
  snapAngle,
  snapToGrid,
  snapValue,
  sub,
  transformWires,
  translation,
  unitAxis,
  cleanPoint,
  JOIN_EPS,
  PLANE_AXES,
  type End,
  type Plane,
  type Pt2,
} from '../domain/wireEdit';
import { isDrawTool, useEditor, type DrawCursor, type Hover } from '../state/editor';
import { useInput } from '../state/input';
import { useApp } from '../state/store';
import { addWires, canEditWires, commitWires, placeFeed, selectConnected } from '../state/wireCommands';

const WIRE_PICK_PX = 6;
const END_PICK_PX = 9;
const FEED_PICK_PX = 10;
/** How far from a wire a dragged source or load still lands on it. */
const FEED_WIRE_PX = 16;
const SNAP_PX = 10;
const DRAG_PX = 4;
/** The view cube in the bottom-right corner keeps its own clicks. */
const VIEW_GIZMO_PX = 112;

const ORIGIN: Point3 = { x: 0, y: 0, z: 0 };
const v3 = (p: Point3) => new Vector3(p.x, p.y, p.z);
const pt = (v: Vector3): Point3 => ({ x: v.x, y: v.y, z: v.z });
const fmt = (v: number) => Number(v.toFixed(4)).toString();
const fmtPoint = (p: Point3) => `(${fmt(p.x)}, ${fmt(p.y)}, ${fmt(p.z)})`;
const PLANE_LABEL: Record<Plane, string> = { xy: 'XY', xz: 'XZ', yz: 'YZ' };

/**
 * The plane free drags and automatic drawing use: the ground plane (XY)
 * unless the camera looks within about 20° of horizontal, then the vertical
 * plane that faces it.
 */
function facingPlane(camera: Camera): Plane {
  const d = camera.getWorldDirection(new Vector3());
  if (Math.abs(d.z) > 0.35) return 'xy';
  return Math.abs(d.y) >= Math.abs(d.x) ? 'xz' : 'yz';
}

/** Mantine keeps some closed modals mounted; an open one has a box. */
/** Whether a key of the viewer's mouse setting is held. */
const held = (setting: 'boxSelect' | 'addToSelection' | 'constrain' | 'followToggle', e: MouseEvent) =>
  isHeld(useInput.getState().mouse[setting], e);

const modalOpen = () => [...document.querySelectorAll('.mantine-Modal-content')].some((e) => e.getClientRects().length > 0);

export interface EditSceneProps {
  /** The antenna's wires (not the stacked copies). */
  wires: Wire[];
  /** Every drawn copy, for picking. */
  display: DisplayWire[];
  /** Where the first stacked copy is drawn: world = wire + offset. */
  offset: Point3;
  /** Grid step in metres. */
  step: number;
  editable: boolean;
  /** Size of the antenna, for the drawing grid. */
  radius: number;
  /** Source and load markers in wire coordinates. */
  feeds: FeedPositions;
  /** The pulse layout, when it fits the wires. */
  layout: LayoutFor | null;
}

type Pending = {
  kind: 'pending';
  at: Pt2;
  hit: Hit | null;
  /** Adds the wire to the selection (or takes it out) instead of selecting it alone. */
  add: boolean;
  box: boolean;
  draw: boolean;
};
type Drag =
  | Pending
  | { kind: 'move'; plane: Plane; through: Point3; anchor: Point3; base: Wire[]; sel: number[] }
  | { kind: 'end'; wire: number; end: End; plane: Plane; origin: Point3; base: Wire[]; sel: number[] }
  | { kind: 'box'; from: Pt2 }
  | { kind: 'feed'; item: FeedRef };

type Hit = { kind: 'wire'; wire: number; grab: Point3 } | { kind: 'end'; wire: number; end: End } | { kind: 'feed'; item: FeedRef };

export function EditScene(props: EditSceneProps) {
  const gizmo = useRef<TransformControlsImpl>(null);
  return (
    <>
      <PointerController {...props} gizmo={gizmo} />
      <EndHandles wires={props.wires} offset={props.offset} />
      <SelectionGizmo {...props} gizmo={gizmo} />
      <DrawPreview offset={props.offset} step={props.step} radius={props.radius} />
      <PulseTicks wires={props.wires} offset={props.offset} layout={props.layout} />
      <IssueMarks offset={props.offset} />
    </>
  );
}

function PointerController({ wires, display, offset, step, editable, feeds, layout, gizmo }: EditSceneProps & { gizmo: RefObject<TransformControlsImpl | null> }) {
  const { camera, gl, size, events } = useThree();
  const latest = useRef({ wires, display, offset, step, editable, feeds, layout, camera, size });
  latest.current = { wires, display, offset, step, editable, feeds, layout, camera, size };

  useEffect(() => {
    const el = (events.connected as HTMLElement | undefined) ?? gl.domElement;
    const raycaster = new Raycaster();
    const ndc = new Vector2();
    const tmp = new Vector3();
    let drag: Drag | null = null;

    const local = (e: MouseEvent): Pt2 => {
      const r = gl.domElement.getBoundingClientRect();
      return { x: e.clientX - r.left, y: e.clientY - r.top };
    };
    const toScreen = (p: Point3): Pt2 | null => {
      const { camera, size } = latest.current;
      tmp.set(p.x, p.y, p.z).project(camera);
      if (tmp.z < -1 || tmp.z > 1) return null;
      return { x: ((tmp.x + 1) / 2) * size.width, y: ((1 - tmp.y) / 2) * size.height };
    };
    /** The world point under `p` on the axis plane through `through` (world). */
    const onPlane = (p: Pt2, plane: Plane, through: Point3): Point3 | null => {
      const { camera, size } = latest.current;
      ndc.set((p.x / size.width) * 2 - 1, -(p.y / size.height) * 2 + 1);
      raycaster.setFromCamera(ndc, camera);
      const n = v3(unitAxis(PLANE_NORMAL[plane]));
      const hit = raycaster.ray.intersectPlane(new ThreePlane(n, -n.dot(v3(through))), new Vector3());
      return hit ? pt(hit) : null;
    };
    const toWorld = (p: Point3) => add(p, latest.current.offset);
    const toWire = (p: Point3) => sub(p, latest.current.offset);

    const pickEnd = (p: Pt2): (Hit & { kind: 'end' }) | null => {
      const { wires } = latest.current;
      let best: (Hit & { kind: 'end' }) | null = null;
      let bestD = END_PICK_PX;
      for (const i of useApp.getState().selectedWires) {
        for (const end of ['start', 'end'] as End[]) {
          const s = wires[i] && toScreen(toWorld(wires[i][end]));
          const d = s ? Math.hypot(s.x - p.x, s.y - p.y) : Infinity;
          if (d < bestD) {
            bestD = d;
            best = { kind: 'end', wire: i, end };
          }
        }
      }
      return best;
    };
    /** The wire under the pointer; `shift` takes the first stacked copy onto the copy hit. */
    const pickWire = (p: Pt2, px = WIRE_PICK_PX): (Hit & { kind: 'wire'; shift: Point3 }) | null => {
      let best: (Hit & { kind: 'wire'; shift: Point3 }) | null = null;
      let bestD = px;
      const selected = new Set(useApp.getState().selectedWires);
      for (const w of latest.current.display) {
        const a = toScreen(w.start);
        const b = toScreen(w.end);
        if (!a || !b) continue;
        // a selected wire wins over one at the same place (a copy over its original)
        const d = distToSegment2(p, a, b) - (selected.has(w.base) ? 1 : 0);
        if (d < bestD) {
          bestD = d;
          const l2 = (b.x - a.x) ** 2 + (b.y - a.y) ** 2;
          const t = l2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / l2));
          best = {
            kind: 'wire',
            wire: w.base,
            grab: add(w.start, scale(sub(w.end, w.start), t)),
            shift: sub(w.start, toWorld(latest.current.wires[w.base].start)),
          };
        }
      }
      return best;
    };

    /** The source or load marker under the pointer. */
    const pickFeed = (p: Pt2): Hit | null => {
      const { feeds } = latest.current;
      let best: Hit | null = null;
      let bestD = FEED_PICK_PX;
      const consider = (kind: FeedRef['kind'], at: (Point3 | null)[]) =>
        at.forEach((q, index) => {
          const s = q && toScreen(toWorld(q));
          const d = s ? Math.hypot(s.x - p.x, s.y - p.y) : Infinity;
          if (d < bestD) {
            bestD = d;
            best = { kind: 'feed', item: { kind, index } };
          }
        });
      // a source wins over a load on the same pulse
      consider('source', feeds.sources);
      consider('load', feeds.loads);
      return best;
    };

    /** The pulse nearest the pointer on the wire under it. */
    const pickPulse = (p: Pt2): { wire: number; pulse: PulsePoint } | null => {
      const { layout, wires } = latest.current;
      const hit = layout && pickWire(p, FEED_WIRE_PX);
      if (!layout || !hit) return null;
      let best: PulsePoint | null = null;
      let bestD = Infinity;
      for (const q of pulsesOnWire(layout.layout, wires, hit.wire)) {
        // measured on the copy under the pointer, though the names follow the first
        const s = toScreen(add(toWorld(q.position), hit.shift));
        const d = s ? Math.hypot(s.x - p.x, s.y - p.y) : Infinity;
        if (d < bestD) {
          bestD = d;
          best = q;
        }
      }
      return best && { wire: hit.wire, pulse: best };
    };

    /**
     * The nearest end or midpoint on screen (wire coordinates), else the
     * nearest point along a wire (at a round distance from its start with
     * the grid on), skipping `except` and the wires that end there.
     */
    const snapTarget = (p: Pt2, except?: Point3): { point: Point3; snap: 'vertex' | 'midpoint' | 'wire'; note?: string } | null => {
      const { wires } = latest.current;
      const ed = useEditor.getState();
      let best: { point: Point3; snap: 'vertex' | 'midpoint' | 'wire'; note?: string } | null = null;
      let bestD = SNAP_PX;
      const consider = (q: Point3, snap: 'vertex' | 'midpoint') => {
        if (except && samePoint(q, except)) return;
        const s = toScreen(toWorld(q));
        if (!s) return;
        // ends win over midpoints at the same distance
        const d = Math.hypot(s.x - p.x, s.y - p.y) + (snap === 'midpoint' ? 2 : 0);
        if (d < bestD) {
          bestD = d;
          best = { point: q, snap };
        }
      };
      for (const w of wires) {
        consider(w.start, 'vertex');
        consider(w.end, 'vertex');
        consider(scale(add(w.start, w.end), 0.5), 'midpoint');
      }
      for (const q of ed.draft) consider(q, 'vertex');
      if (best) return best;
      const hit = pickWire(p, SNAP_PX);
      const w = hit && wires[hit.wire];
      if (!hit || !w || (except && (samePoint(w.start, except) || samePoint(w.end, except)))) return null;
      const len = distance(w.start, w.end);
      if (len < JOIN_EPS) return null;
      const grab = toWire(sub(hit.grab, hit.shift));
      let d = Math.min(len, Math.max(0, len * along(w, grab).t));
      if (ed.gridSnap) d = Math.min(len, snapValue(d, latest.current.step));
      return { point: cleanPoint(add(w.start, scale(sub(w.end, w.start), d / len))), snap: 'wire', note: `ワイヤー ${hit.wire + 1} 上・始点から ${fmt(d)} m` };
    };

    const drawPlane = (): Plane => {
      const ed = useEditor.getState();
      return ed.draftPlane ?? (ed.plane === 'auto' ? facingPlane(latest.current.camera) : ed.plane);
    };

    const drawCursor = (p: Pt2, constrain: boolean): DrawCursor | null => {
      const ed = useEditor.getState();
      const plane = drawPlane();
      const last = ed.draft.at(-1);
      if (ed.vertexSnap && !(constrain && last)) {
        const s = snapTarget(p);
        if (s) return { point: s.point, plane, snap: s.snap, note: s.note };
      }
      const q = onPlane(p, plane, toWorld(last ?? ORIGIN));
      if (!q) return null;
      let point = toWire(q);
      if (constrain && last) return { point: snapAngle(last, point, plane, 15), plane, snap: 'free' };
      if (ed.gridSnap) point = snapToGrid(point, plane, latest.current.step);
      return { point: cleanPoint(point), plane, snap: ed.gridSnap ? 'grid' : 'free' };
    };

    const like = () => {
      const { wires } = latest.current;
      const active = useApp.getState().selectedWire;
      return (active != null ? wires[active] : undefined) ?? wires.at(-1);
    };

    /** A click of a drawing tool at `point`. */
    const placePoint = (point: Point3, plane: Plane) => {
      if (!canEditWires()) return;
      const ed = useEditor.getState();
      const draft = ed.draft;
      if (draft.length === 0) {
        ed.set({ draft: [point], draftPlane: ed.tool === 'wire' ? null : plane, typed: '' });
        return;
      }
      const first = draft[0];
      const last = draft[draft.length - 1];
      if (ed.tool === 'wire') {
        if (samePoint(point, last)) return;
        addWires(polylineWires([last, point], like()));
        // clicking the first corner closes the loop and ends the line
        const closed = draft.length >= 2 && samePoint(point, first);
        ed.set({ draft: closed ? [] : [...draft, point], typed: '' });
        return;
      }
      const p = ed.draftPlane ?? plane;
      const [u, v] = PLANE_AXES[p];
      if (ed.tool === 'rect') {
        if (Math.abs(point[u] - first[u]) < JOIN_EPS || Math.abs(point[v] - first[v]) < JOIN_EPS) return;
        addWires(polylineWires(rectanglePoints(first, point, p), like()));
      } else {
        const corner = { ...point, [PLANE_NORMAL[p]]: first[PLANE_NORMAL[p]] };
        if (distance(corner, first) < JOIN_EPS) return;
        addWires(polylineWires(polygonPoints(first, corner, p, ed.sides), like()));
      }
      ed.set({ draft: [], draftPlane: null, typed: '' });
    };

    /** Enter after typing a length: the next point at that distance towards the pointer. */
    const placeTyped = () => {
      const ed = useEditor.getState();
      const len = Number(ed.typed);
      const from = ed.draft.at(-1);
      if (!from || !ed.cursor || !(Number.isFinite(len) && len !== 0) || ed.tool === 'rect') return;
      const plane = ed.draftPlane ?? ed.cursor.plane;
      let dir = sub(ed.cursor.point, from);
      if (ed.tool === 'polygon') dir = { ...dir, [PLANE_NORMAL[plane]]: 0 };
      if (norm(dir) < JOIN_EPS) dir = unitAxis(PLANE_AXES[plane][0]);
      placePoint(cleanPoint(add(from, scale(dir, len / norm(dir)))), plane);
    };

    const setHover = (h: Hover) => {
      const cur = useEditor.getState().hover;
      const same = cur === h || (cur && h && cur.kind === h.kind && cur.wire === h.wire && (cur.kind !== 'end' || (h.kind === 'end' && cur.end === h.end)));
      if (!same) useEditor.getState().set({ hover: h });
      el.style.cursor = !h ? '' : h.kind === 'end' ? 'move' : 'pointer';
    };

    const updateHover = (e: PointerEvent) => {
      const ed = useEditor.getState();
      const p = local(e);
      if (isDrawTool(ed.tool)) {
        el.style.cursor = 'crosshair';
        const cursor = drawCursor(p, held('constrain', e));
        const last = ed.draft.at(-1);
        let hud: string | null = cursor?.note ?? null;
        if (cursor && last) {
          hud =
            ed.tool === 'wire'
              ? `長さ ${fmt(distance(last, cursor.point))} m`
              : ed.tool === 'rect'
                ? (() => {
                    const [u, v] = PLANE_AXES[ed.draftPlane ?? cursor.plane];
                    return `${fmt(Math.abs(cursor.point[u] - last[u]))} × ${fmt(Math.abs(cursor.point[v] - last[v]))} m`;
                  })()
                : `半径 ${fmt(distance(last, cursor.point))} m`;
          if (cursor.note) hud += `（${cursor.note}）`;
        }
        ed.set({ cursor, hud });
        return;
      }
      if (p.x > latest.current.size.width - VIEW_GIZMO_PX && p.y > latest.current.size.height - VIEW_GIZMO_PX) return setHover(null);
      if (ed.tool === 'feed') {
        const f = pickFeed(p);
        const item = f?.kind === 'feed' ? f.item : null;
        const cur = ed.feedHover;
        if (cur?.kind !== item?.kind || cur?.index !== item?.index) ed.set({ feedHover: item });
        // the wire under the pointer shows its pulses
        const w = item ? null : pickWire(p, FEED_WIRE_PX);
        setHover(w && { kind: 'wire', wire: w.wire });
        el.style.cursor = item ? 'grab' : '';
        return;
      }
      const h = pickEnd(p) ?? pickWire(p);
      setHover(h && (h.kind === 'end' ? h : { kind: 'wire', wire: h.wire }));
    };

    const startDrag = (d: Pending, e: PointerEvent) => {
      const app = useApp.getState();
      const ed = useEditor.getState();
      if (d.box) {
        drag = { kind: 'box', from: d.at };
        return;
      }
      const hit = d.hit!;
      if (!latest.current.editable) {
        drag = null;
        return;
      }
      if (hit.kind === 'feed') {
        drag = { kind: 'feed', item: hit.item };
        ed.set({ hover: null, feedHover: null, feedDrag: { item: hit.item, to: null } });
        el.style.cursor = 'grabbing';
        updateDrag(e);
        return;
      }
      const plane = facingPlane(latest.current.camera);
      if (hit.kind === 'end') {
        drag = {
          kind: 'end',
          wire: hit.wire,
          end: hit.end,
          plane,
          origin: latest.current.wires[hit.wire][hit.end],
          base: latest.current.wires,
          sel: app.selectedWires,
        };
      } else {
        let sel = app.selectedWires;
        if (!sel.includes(hit.wire)) {
          sel = d.add ? [...sel, hit.wire] : [hit.wire];
          app.setSelection(sel, hit.wire);
        }
        const anchor = onPlane(d.at, plane, hit.grab);
        if (!anchor) {
          drag = null;
          return;
        }
        drag = { kind: 'move', plane, through: hit.grab, anchor, base: latest.current.wires, sel };
      }
      ed.set({ hover: null });
      updateDrag(e);
    };

    const updateDrag = (e: PointerEvent) => {
      if (!drag) return;
      const ed = useEditor.getState();
      const p = local(e);
      const step = latest.current.step;
      const constrain = held('constrain', e);
      const follow = ed.follow !== held('followToggle', e);
      if (drag.kind === 'feed') {
        const label = feedLabel(drag.item);
        if (!latest.current.layout) {
          ed.set({ hud: `${label}: ワイヤーをパルスに分けられないため移動できません（計算できる形状か確認してください）` });
          return;
        }
        const target = pickPulse(p);
        ed.set({
          hover: target && { kind: 'wire', wire: target.wire },
          feedDrag: { item: drag.item, to: target?.pulse ?? null },
          hud: target
            ? `${label} → ${describePulse(target.pulse, latest.current.wires[target.wire])}`
            : `${label}: ワイヤーの上で離すとそのパルスに移動します`,
        });
      } else if (drag.kind === 'box') {
        ed.set({ hud: null, box: { x0: Math.min(drag.from.x, p.x), y0: Math.min(drag.from.y, p.y), x1: Math.max(drag.from.x, p.x), y1: Math.max(drag.from.y, p.y) } });
      } else if (drag.kind === 'move') {
        const q = onPlane(p, drag.plane, drag.through);
        if (!q) return;
        let d = sub(q, drag.anchor);
        if (constrain) d = dominantAxis(d);
        if (ed.gridSnap) d = cleanPoint({ x: snapValue(d.x, step), y: snapValue(d.y, step), z: snapValue(d.z, step) });
        ed.set({
          preview: transformWires(drag.base, drag.sel, translation(d), follow),
          hud: `移動 ΔX ${fmt(d.x)}  ΔY ${fmt(d.y)}  ΔZ ${fmt(d.z)} m（${PLANE_LABEL[drag.plane]} 面）`,
        });
      } else if (drag.kind === 'end') {
        let target: Point3 | null = null;
        let snapped = '';
        if (ed.vertexSnap && !constrain) {
          const s = snapTarget(p, drag.origin);
          if (s) {
            target = s.point;
            snapped = s.snap === 'vertex' ? '（端点に吸着）' : s.snap === 'midpoint' ? '（中点に吸着）' : `（${s.note}）`;
          }
        }
        if (!target) {
          const q = onPlane(p, drag.plane, toWorld(drag.origin));
          if (!q) return;
          target = toWire(q);
          if (constrain) {
            const d = dominantAxis(sub(target, drag.origin));
            const k = (['x', 'y', 'z'] as const).find((a) => d[a] !== 0) ?? 'x';
            target = { ...drag.origin, [k]: ed.gridSnap ? snapValue(target[k], step) : target[k] };
          } else if (ed.gridSnap) target = snapToGrid(target, drag.plane, step);
        }
        const wires = moveEndpoint(drag.base, drag.wire, drag.end, target, follow, drag.sel);
        const w = wires[drag.wire];
        ed.set({ preview: wires, hud: `端点 ${fmtPoint(target)}  長さ ${fmt(distance(w.start, w.end))} m${snapped}` });
      }
    };

    const endDrag = (cancel: boolean) => {
      const d = drag;
      drag = null;
      window.removeEventListener('pointermove', onWindowMove);
      window.removeEventListener('pointerup', onWindowUp);
      const ed = useEditor.getState();
      const preview = ed.preview;
      const to = ed.feedDrag?.to;
      ed.set({ preview: null, hud: null, box: null, feedDrag: null });
      if (d?.kind === 'feed') {
        el.style.cursor = '';
        ed.set({ hover: null });
      }
      if (!cancel && d && (d.kind === 'move' || d.kind === 'end') && preview) commitWires(preview);
      if (!cancel && d?.kind === 'feed' && to) placeFeed(d.item, to.name);
      return d;
    };

    const onDown = (e: PointerEvent) => {
      if (drag) return;
      const L = latest.current;
      const ed = useEditor.getState();
      const p = local(e);
      if (p.x > L.size.width - VIEW_GIZMO_PX && p.y > L.size.height - VIEW_GIZMO_PX) return;
      // the gizmo under the pointer takes the drag (`axis` is set while it is hovered)
      if ((gizmo.current as unknown as { axis: string | null } | null)?.axis) return;
      const add = held('addToSelection', e);
      if (ed.tool === 'feed') {
        if (e.button !== 0) return;
        // anything but a marker turns the camera
        const hit = pickFeed(p);
        if (!hit) return;
        e.stopPropagation();
        drag = { kind: 'pending', at: p, hit, add: false, box: false, draw: false };
      } else if (isDrawTool(ed.tool)) {
        if (e.button === 2 && ed.draft.length) {
          // right click ends the line instead of panning
          e.stopPropagation();
          ed.cancelDraft();
          return;
        }
        if (e.button !== 0) return;
        // the camera still turns when the pointer is dragged
        drag = { kind: 'pending', at: p, hit: null, add, box: false, draw: true };
      } else {
        if (e.button !== 0) return;
        const hit = pickEnd(p) ?? pickWire(p);
        const box = !hit && held('boxSelect', e);
        if (hit || box) e.stopPropagation();
        drag = { kind: 'pending', at: p, hit, add, box, draw: false };
      }
      window.addEventListener('pointermove', onWindowMove);
      window.addEventListener('pointerup', onWindowUp);
    };

    const onWindowMove = (e: PointerEvent) => {
      if (!drag) return;
      if (drag.kind === 'pending') {
        const p = local(e);
        if (Math.hypot(p.x - drag.at.x, p.y - drag.at.y) < DRAG_PX) return;
        // a drag that started on empty space turns the camera
        if (drag.draw || (!drag.hit && !drag.box)) return void endDrag(true);
        startDrag(drag, e);
        return;
      }
      updateDrag(e);
    };

    const onWindowUp = (e: PointerEvent) => {
      const box = useEditor.getState().box;
      const d = endDrag(false);
      if (!d) return;
      const app = useApp.getState();
      if (d.kind === 'box' && box) {
        const hits = new Set<number>();
        for (const w of latest.current.display) {
          const a = toScreen(w.start);
          const b = toScreen(w.end);
          if (a && b && segmentHitsRect(a, b, box)) hits.add(w.base);
        }
        app.setSelection([...app.selectedWires, ...hits]);
      } else if (d.kind === 'pending') {
        if (d.hit?.kind === 'feed') return;
        if (d.draw) {
          const c = drawCursor(local(e), held('constrain', e));
          if (c) placePoint(c.point, c.plane);
        } else if (d.hit) {
          if (d.add) app.toggleWire(d.hit.wire);
          else app.selectWire(d.hit.wire);
        } else if (!d.box) {
          app.setSelection([]);
        }
      }
    };

    const onMove = (e: PointerEvent) => {
      if (!drag && e.buttons === 0) updateHover(e);
    };
    const onLeave = () => {
      if (drag) return;
      setHover(null);
      useEditor.getState().set({ cursor: null, feedHover: null });
    };
    const onDblClick = (e: MouseEvent) => {
      const ed = useEditor.getState();
      if (isDrawTool(ed.tool)) {
        ed.cancelDraft();
        return;
      }
      if (ed.tool === 'feed') return;
      const hit = pickWire(local(e));
      if (hit) selectConnected(hit.wire, held('addToSelection', e));
    };
    const onContextMenu = (e: MouseEvent) => {
      if (isDrawTool(useEditor.getState().tool)) e.preventDefault();
    };
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable)) return;
      if (modalOpen()) return;
      const ed = useEditor.getState();
      const stop = () => {
        e.preventDefault();
        e.stopPropagation();
      };
      if (e.key === 'Escape') {
        if (drag && drag.kind !== 'pending') {
          endDrag(true);
          stop();
        } else if (ed.cancelDraft()) stop();
        else if (isDrawTool(ed.tool)) {
          ed.setTool('select');
          stop();
        }
        return;
      }
      if (!isDrawTool(ed.tool) || ed.draft.length === 0) return;
      if (/^[0-9.]$/.test(e.key) || (e.key === '-' && ed.typed === '')) {
        ed.set({ typed: ed.typed + e.key });
        stop();
      } else if (e.key === 'Backspace' || e.key === 'Delete') {
        ed.set({ typed: ed.typed.slice(0, -1) });
        stop();
      } else if (e.key === 'Enter') {
        if (ed.typed) placeTyped();
        else ed.cancelDraft();
        stop();
      }
    };

    el.addEventListener('pointerdown', onDown, true);
    el.addEventListener('pointermove', onMove);
    el.addEventListener('pointerleave', onLeave);
    el.addEventListener('dblclick', onDblClick);
    el.addEventListener('contextmenu', onContextMenu);
    window.addEventListener('keydown', onKey, true);
    return () => {
      endDrag(true);
      el.style.cursor = '';
      el.removeEventListener('pointerdown', onDown, true);
      el.removeEventListener('pointermove', onMove);
      el.removeEventListener('pointerleave', onLeave);
      el.removeEventListener('dblclick', onDblClick);
      el.removeEventListener('contextmenu', onContextMenu);
      window.removeEventListener('keydown', onKey, true);
    };
  }, [events.connected, gl, gizmo]);

  return null;
}

/** Ends of the selected wires, which can be dragged. */
function EndHandles({ wires, offset }: { wires: Wire[]; offset: Point3 }) {
  const preview = useEditor((s) => s.preview);
  const tool = useEditor((s) => s.tool);
  const hover = useEditor((s) => s.hover);
  const selected = useApp((s) => s.selectedWires);
  const invalidate = useThree((s) => s.invalidate);
  const shown = preview ?? wires;
  const points = useMemo(() => {
    const out: { p: Point3; hot: boolean }[] = [];
    for (const i of selected) {
      const w = shown[i];
      if (!w) continue;
      for (const end of ['start', 'end'] as End[]) {
        const p = add(w[end], offset);
        const hot = hover?.kind === 'end' && hover.wire === i && hover.end === end;
        const dup = out.findIndex((o) => samePoint(o.p, p));
        if (dup >= 0) out[dup].hot ||= hot;
        else out.push({ p, hot });
      }
    }
    return out;
  }, [shown, selected, offset, hover]);
  useEffect(() => invalidate());
  if (isDrawTool(tool) || points.length === 0 || points.length > 400) return null;
  return (
    <>
      {points.map((h, i) => (
        <Html key={i} position={[h.p.x, h.p.y, h.p.z]} center zIndexRange={[5, 0]} style={{ pointerEvents: 'none' }}>
          <div className={`end-handle${h.hot ? ' hot' : ''}`} />
        </Html>
      ))}
    </>
  );
}

/** The move / rotate gizmo at the centre of the selection. */
function SelectionGizmo({ wires, offset, step, editable, gizmo }: EditSceneProps & { gizmo: RefObject<TransformControlsImpl | null> }) {
  const tool = useEditor((s) => s.tool);
  const gridSnap = useEditor((s) => s.gridSnap);
  const angleSnap = useEditor((s) => s.angleSnap);
  const selected = useApp((s) => s.selectedWires);
  const invalidate = useThree((s) => s.invalidate);
  const pivot = useRef<Group>(null);
  const start = useRef<{ base: Wire[]; sel: number[]; center: Point3; pos: Vector3 } | null>(null);
  const center = useMemo(() => selectionCenter(wires, selected), [wires, selected]);
  const active = (tool === 'move' || tool === 'rotate') && center != null && editable;

  const place = () => {
    const g = pivot.current;
    if (!g || !center || start.current) return;
    g.position.set(center.x + offset.x, center.y + offset.y, center.z + offset.z);
    g.quaternion.identity();
    invalidate();
  };
  useLayoutEffect(place);

  if (!active) return null;
  const changed = () => {
    const s = start.current;
    const g = pivot.current;
    if (!s || !g) return;
    const ed = useEditor.getState();
    if (tool === 'move') {
      const d = pt(g.position.clone().sub(s.pos));
      ed.set({ preview: transformWires(s.base, s.sel, translation(d), ed.follow), hud: `移動 ΔX ${fmt(d.x)}  ΔY ${fmt(d.y)}  ΔZ ${fmt(d.z)} m` });
    } else {
      const q = g.quaternion.clone();
      const c = v3(s.center);
      const f = (p: Point3) => pt(v3(p).sub(c).applyQuaternion(q).add(c));
      const deg = (2 * Math.acos(Math.min(1, Math.abs(q.w))) * 180) / Math.PI;
      ed.set({ preview: transformWires(s.base, s.sel, f, ed.follow), hud: `回転 ${fmt(deg)}°` });
    }
  };
  return (
    <>
      <group ref={pivot} />
      <TransformControls
        ref={gizmo}
        object={pivot as RefObject<Group>}
        mode={tool === 'rotate' ? 'rotate' : 'translate'}
        space="world"
        size={1.2}
        translationSnap={gridSnap ? step : null}
        rotationSnap={angleSnap ? Math.PI / 12 : null}
        onMouseDown={() => {
          const g = pivot.current;
          if (!g || !center) return;
          start.current = { base: wires, sel: selected, center, pos: g.position.clone() };
        }}
        onObjectChange={changed}
        onMouseUp={() => {
          start.current = null;
          const ed = useEditor.getState();
          const preview = ed.preview;
          ed.set({ preview: null, hud: null });
          if (preview) commitWires(preview);
          place();
        }}
      />
    </>
  );
}

/** The shape being drawn, the snap marker and the grid of the drawing plane. */
function DrawPreview({ offset, step, radius }: { offset: Point3; step: number; radius: number }) {
  const tool = useEditor((s) => s.tool);
  const draft = useEditor((s) => s.draft);
  const draftPlane = useEditor((s) => s.draftPlane);
  const cursor = useEditor((s) => s.cursor);
  const sides = useEditor((s) => s.sides);
  const gridSnap = useEditor((s) => s.gridSnap);
  const invalidate = useThree((s) => s.invalidate);
  useEffect(() => invalidate());
  if (!isDrawTool(tool)) return null;

  const w = (p: Point3): [number, number, number] => [p.x + offset.x, p.y + offset.y, p.z + offset.z];
  const plane = draftPlane ?? cursor?.plane ?? 'xy';
  const last = draft.at(-1);
  let outline: Point3[] | null = null;
  if (last && cursor) {
    if (tool === 'wire') outline = [last, cursor.point];
    else if (tool === 'rect') outline = rectanglePoints(draft[0], cursor.point, plane);
    else {
      const corner = { ...cursor.point, [PLANE_NORMAL[plane]]: draft[0][PLANE_NORMAL[plane]] };
      if (distance(corner, draft[0]) > JOIN_EPS) outline = polygonPoints(draft[0], corner, plane, sides);
    }
  }
  // the grid lies on the drawing plane through the point drawn from
  const through = last ?? (cursor?.snap === 'vertex' || cursor?.snap === 'midpoint' || cursor?.snap === 'wire' ? cursor.point : ORIGIN);
  const normal = PLANE_NORMAL[plane];
  const center = { ...(cursor?.point ?? through), [normal]: through[normal] };
  const gridCenter = gridSnap ? snapToGrid(center, plane, step * 10) : center;
  const extent = Math.max(step * 40, radius * 2.5);
  const rotation: [number, number, number] = plane === 'xy' ? [Math.PI / 2, 0, 0] : plane === 'yz' ? [0, 0, Math.PI / 2] : [0, 0, 0];
  return (
    <>
      <Grid
        position={w(gridCenter)}
        rotation={rotation}
        args={[extent, extent]}
        cellSize={step}
        sectionSize={step * 10}
        cellColor="#86b6ef"
        sectionColor="#2a78d6"
        cellThickness={0.5}
        sectionThickness={0.9}
        fadeDistance={extent * 1.5}
        fadeStrength={2}
        side={2}
      />
      {draft.length > 1 && tool === 'wire' && <Line points={draft.map(w)} color="#eb6834" lineWidth={2.5} />}
      {outline && <Line points={outline.map(w)} color="#eb6834" lineWidth={2} dashed dashSize={step / 2} gapSize={step / 3} />}
      {draft.map((p, i) => (
        <Html key={i} position={w(p)} center zIndexRange={[5, 0]} style={{ pointerEvents: 'none' }}>
          <div className="end-handle" />
        </Html>
      ))}
      {cursor && (
        <Html position={w(cursor.point)} center zIndexRange={[6, 0]} style={{ pointerEvents: 'none' }}>
          <div className={`snap-marker ${cursor.snap}`} />
        </Html>
      )}
    </>
  );
}

/** The pulses of the wire under the pointer, where the feed tool can put a source or load. */
function PulseTicks({ wires, offset, layout }: { wires: Wire[]; offset: Point3; layout: LayoutFor | null }) {
  const tool = useEditor((s) => s.tool);
  const wire = useEditor((s) => (s.hover?.kind === 'wire' ? s.hover.wire : null));
  const target = useEditor((s) => s.feedDrag?.to?.pulse ?? null);
  const invalidate = useThree((s) => s.invalidate);
  const ticks = useMemo(() => (layout && wire != null ? pulsesOnWire(layout.layout, wires, wire) : []), [layout, wires, wire]);
  useEffect(() => invalidate());
  if (tool !== 'feed' || ticks.length === 0 || ticks.length > 400) return null;
  return (
    <>
      {ticks.map((q) => (
        <Html key={q.pulse} position={[q.position.x + offset.x, q.position.y + offset.y, q.position.z + offset.z]} center zIndexRange={[5, 0]} style={{ pointerEvents: 'none' }}>
          <div className={`pulse-tick${q.pulse === target ? ' hot' : ''}`} />
        </Html>
      ))}
    </>
  );
}

/** The places the connection check lists, while its list is open. */
function IssueMarks({ offset }: { offset: Point3 }) {
  const marks = useEditor((s) => s.issueMarks);
  const hot = useEditor((s) => s.issueHot);
  const invalidate = useThree((s) => s.invalidate);
  useEffect(() => invalidate());
  return (
    <>
      {marks.map((p, i) => (
        <Html key={i} position={[p.x + offset.x, p.y + offset.y, p.z + offset.z]} center zIndexRange={[7, 0]} style={{ pointerEvents: 'none' }}>
          <div className={`issue-mark${i === hot ? ' hot' : ''}`} />
        </Html>
      ))}
    </>
  );
}
