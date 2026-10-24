// Dragging rows of the wire list to reorder the wires and move them in and
// out of groups. Pointer events rather than HTML drag and drop, which the
// webviews of the desktop app handle differently. The list scrolls while the
// pointer is near its top or bottom edge.
import { useEffect, useRef, useState, type RefObject } from 'react';
import { dropTarget, type Dragged, type Drop, type DropSpot, type ListRow } from '../domain/wireGroups';

/** Pixels a press moves before it becomes a drag (less is a click). */
const THRESHOLD = 4;
/** The list scrolls while the pointer is this close to its edge… */
const EDGE = 36;
/** …by up to this many pixels a frame. */
const SPEED = 14;
/** Indent of the drop line inside a group, past the row number. */
const INDENT = 34;

export interface DragStart {
  /** The wires that move, in list order. */
  wires: number[];
  dragged: Dragged;
  label: string;
  /** Runs once the press becomes a drag (to select what is dragged). */
  begin?: () => void;
}

/** What the list shows during a drag: where the line goes and the label by the pointer. */
export interface DragView {
  x: number;
  y: number;
  label: string;
  line: { top: number; left: number; width: number } | null;
}

export function useWireDrag(
  body: RefObject<HTMLTableSectionElement | null>,
  rows: ListRow[],
  wireCount: number,
  collapsed: (name: string) => boolean,
  onDrop: (start: DragStart, drop: Drop) => void,
) {
  const [view, setView] = useState<DragView | null>(null);
  // the listeners outlive renders; they read the latest list from here
  const latest = useRef({ rows, wireCount, collapsed, onDrop });
  latest.current = { rows, wireCount, collapsed, onDrop };
  const cleanup = useRef<(() => void) | null>(null);
  const dragged = useRef(false);

  useEffect(() => () => cleanup.current?.(), []);

  const spotAt = (y: number): DropSpot | null => {
    const trs = body.current?.querySelectorAll<HTMLTableRowElement>('tr[data-row]');
    if (!trs?.length) return null;
    for (const tr of trs) {
      const r = tr.getBoundingClientRect();
      if (y < r.bottom) return { row: Number(tr.dataset.row), lower: y >= r.top + r.height / 2 };
    }
    return 'end';
  };

  const lineFor = (drop: Drop | null): DragView['line'] => {
    const tb = body.current;
    const tr = drop && tb?.querySelector(`tr[data-row="${drop.line.row}"]`);
    if (!tb || !tr || !drop) return null;
    const r = tr.getBoundingClientRect();
    // within the part of the list that is scrolled into view
    const shown = tb.closest('.mantine-ScrollArea-viewport')?.getBoundingClientRect() ?? r;
    const left = Math.max(tb.getBoundingClientRect().left + (drop.line.indent ? INDENT : 0), shown.left);
    return { top: (drop.line.edge === 'top' ? r.top : r.bottom) - 1, left, width: Math.min(r.right, shown.right) - left };
  };

  /** Call on pointerdown on a row; a click that does not move stays a click. */
  const start = (e: React.PointerEvent, s: DragStart) => {
    if (e.button !== 0 || cleanup.current) return;
    const x0 = e.clientX;
    const y0 = e.clientY;
    const viewport = body.current?.closest<HTMLElement>('.mantine-ScrollArea-viewport') ?? null;
    let active = false;
    let x = x0;
    let y = y0;
    let drop: Drop | null = null;
    let frame = 0;

    const update = () => {
      const spot = spotAt(y);
      const { rows: r, wireCount: n, collapsed: c } = latest.current;
      drop = spot == null ? null : dropTarget(r, spot, n, s.dragged, c);
      setView({ x, y, label: s.label, line: lineFor(drop) });
    };
    const scroll = () => {
      frame = requestAnimationFrame(scroll);
      if (!viewport) return;
      const r = viewport.getBoundingClientRect();
      const d = y < r.top + EDGE ? -(r.top + EDGE - y) : y > r.bottom - EDGE ? y - (r.bottom - EDGE) : 0;
      if (d === 0) return;
      const before = viewport.scrollTop;
      viewport.scrollTop += Math.sign(d) * Math.min(SPEED, Math.ceil(Math.abs(d) / 2));
      if (viewport.scrollTop !== before) update();
    };
    const move = (ev: PointerEvent) => {
      x = ev.clientX;
      y = ev.clientY;
      if (!active) {
        if (Math.hypot(x - x0, y - y0) < THRESHOLD) return;
        active = true;
        dragged.current = true;
        document.body.classList.add('wire-dragging');
        window.getSelection()?.removeAllRanges();
        s.begin?.();
        frame = requestAnimationFrame(scroll);
      }
      update();
    };
    const end = (commit: boolean) => {
      cleanup.current?.();
      if (commit && active && drop) latest.current.onDrop(s, drop);
      // the click that follows the release is not a click on the row
      setTimeout(() => (dragged.current = false));
    };
    // a press on a row number would otherwise select the text of the cells it passes
    const noSelect = (ev: Event) => ev.preventDefault();
    const up = () => end(true);
    const cancel = () => end(false);
    const key = (ev: KeyboardEvent) => {
      if (ev.key !== 'Escape' || !active) return;
      ev.preventDefault();
      ev.stopPropagation();
      end(false);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    window.addEventListener('keydown', key, true);
    document.addEventListener('selectstart', noSelect);
    cleanup.current = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel);
      window.removeEventListener('keydown', key, true);
      document.removeEventListener('selectstart', noSelect);
      cancelAnimationFrame(frame);
      document.body.classList.remove('wire-dragging');
      setView(null);
      cleanup.current = null;
    };
  };

  /** Whether the click being handled ends a drag (and is not a click). */
  const wasDrag = () => dragged.current;

  return { view, start, wasDrag };
}
