// State of direct editing in the 3D view: the tool, snapping, the wire
// clipboard and what a drag in progress shows. None of it is part of the
// document or its undo history.
import { create } from 'zustand';
import type { Point3, PulsePoint, Wire } from '../api/types';
import type { FeedRef } from '../domain/feed';
import type { End, Plane } from '../domain/wireEdit';

export type Tool = 'select' | 'move' | 'rotate' | 'wire' | 'rect' | 'polygon' | 'feed';
export const DRAW_TOOLS: Tool[] = ['wire', 'rect', 'polygon'];
export const isDrawTool = (t: Tool) => DRAW_TOOLS.includes(t);

/** The drawing plane; `auto` faces the camera. */
export type PlaneMode = 'auto' | Plane;

export type Hover = { kind: 'wire'; wire: number } | { kind: 'end'; wire: number; end: End } | null;

/** The point under the pointer while drawing, and what it snapped to. */
export interface DrawCursor {
  point: Point3;
  plane: Plane;
  snap: 'vertex' | 'midpoint' | 'grid' | 'free';
}

export interface EditorState {
  tool: Tool;
  plane: PlaneMode;
  gridSnap: boolean;
  /** Grid step in metres; null picks one from the size of the antenna. */
  step: number | null;
  /** Spacing of the ground (or Z = 0) grid in metres; null follows the grid step. */
  groundStep: number | null;
  vertexSnap: boolean;
  /** Rotation in 15° steps, drawing directions with Shift. */
  angleSnap: boolean;
  /** Ends of other wires that meet a moved end move with it. */
  follow: boolean;
  /** Corners of the polygon tool. */
  sides: number;

  clipboard: Wire[];
  /** The wires shown instead of the antenna's while a drag is in progress. */
  preview: Wire[] | null;
  hover: Hover;
  /** Points of the shape being drawn (a polyline keeps all its corners). */
  draft: Point3[];
  /** The plane a rectangle or polygon is drawn on, fixed by its first click. */
  draftPlane: Plane | null;
  cursor: DrawCursor | null;
  /** A length typed while drawing. */
  typed: string;
  /** The source or load marker under the pointer (feed tool). */
  feedHover: FeedRef | null;
  /** The source or load being dragged and the pulse it would go on. */
  feedDrag: { item: FeedRef; to: PulsePoint | null } | null;
  /** Read-out of the drag in progress (distance, angle). */
  hud: string | null;
  /** The box being dragged for selection, in canvas pixels. */
  box: { x0: number; y0: number; x1: number; y1: number } | null;
  transformOpen: boolean;

  set: (patch: Partial<EditorState>) => void;
  setTool: (tool: Tool) => void;
  /** Drops the shape being drawn; false when there was none. */
  cancelDraft: () => boolean;
}

export const useEditor = create<EditorState>()((set, get) => ({
  tool: 'select',
  plane: 'auto',
  gridSnap: true,
  step: null,
  groundStep: null,
  vertexSnap: true,
  angleSnap: true,
  follow: true,
  sides: 8,
  clipboard: [],
  preview: null,
  hover: null,
  draft: [],
  draftPlane: null,
  cursor: null,
  typed: '',
  feedHover: null,
  feedDrag: null,
  hud: null,
  box: null,
  transformOpen: false,

  set: (patch) => set(patch),
  setTool: (tool) => set({ tool, draft: [], draftPlane: null, typed: '', cursor: null, hud: null, feedHover: null, feedDrag: null }),
  cancelDraft: () => {
    if (get().draft.length === 0) return false;
    set({ draft: [], draftPlane: null, typed: '' });
    return true;
  },
}));
