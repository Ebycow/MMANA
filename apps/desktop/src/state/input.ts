// The viewer's keyboard shortcuts and mouse settings, and the hook that runs
// commands from the keyboard. Remembered per viewer like the display
// settings; not part of the document.
import { useEffect, useEffectEvent } from 'react';
import { create } from 'zustand';
import {
  COMMANDS,
  DEFAULT_BINDINGS,
  DEFAULT_MOUSE,
  canonical,
  conflicts,
  formatKey,
  heldText,
  matches,
  type Bindings,
  type CommandId,
  type MouseSettings,
} from '../domain/shortcuts';

export interface InputState {
  bindings: Bindings;
  mouse: MouseSettings;
  /** The settings dialog is open; shortcuts wait while it is. */
  dialogOpen: boolean;
  /** Adds a key to a command, taking it from any command that had it; returns those. */
  addKey: (id: CommandId, key: string) => CommandId[];
  removeKey: (id: CommandId, key: string) => void;
  resetKeys: (id?: CommandId) => void;
  setMouse: (patch: Partial<MouseSettings>) => void;
  resetMouse: () => void;
  setDialogOpen: (open: boolean) => void;
}

const KEY = 'mmana.input';

type Saved = { bindings: Bindings; mouse: MouseSettings };

function load(): Saved {
  const defaults = { bindings: DEFAULT_BINDINGS, mouse: DEFAULT_MOUSE };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return defaults;
    const saved = JSON.parse(raw) as Partial<Saved>;
    // commands added since keep their default keys
    const bindings = { ...DEFAULT_BINDINGS };
    for (const c of COMMANDS) {
      const keys = saved.bindings?.[c.id];
      if (Array.isArray(keys)) bindings[c.id] = keys.filter((k) => typeof k === 'string' && canonical(k) != null);
    }
    const mouse = { ...DEFAULT_MOUSE, ...saved.mouse, buttons: { ...DEFAULT_MOUSE.buttons, ...saved.mouse?.buttons } };
    return { bindings, mouse };
  } catch {
    return defaults;
  }
}

export const useInput = create<InputState>()((set, get) => {
  const save = (patch: Partial<Saved>) => {
    set(patch);
    const { bindings, mouse } = get();
    try {
      localStorage.setItem(KEY, JSON.stringify({ bindings, mouse }));
    } catch {
      // not remembered; the defaults apply next time
    }
  };
  return {
    ...load(),
    dialogOpen: false,
    addKey: (id, key) => {
      const k = canonical(key);
      if (!k) return [];
      const bindings = { ...get().bindings };
      const taken = conflicts(bindings, k, id);
      for (const other of taken) bindings[other] = bindings[other].filter((x) => canonical(x) !== k);
      if (!bindings[id].some((x) => canonical(x) === k)) bindings[id] = [...bindings[id], k];
      save({ bindings });
      return taken;
    },
    removeKey: (id, key) => save({ bindings: { ...get().bindings, [id]: get().bindings[id].filter((k) => k !== key) } }),
    resetKeys: (id) => {
      if (!id) return save({ bindings: DEFAULT_BINDINGS });
      // a default key another command has taken in the meantime stays there
      const bindings = get().bindings;
      const keys = DEFAULT_BINDINGS[id].filter((k) => conflicts(bindings, k, id).length === 0);
      save({ bindings: { ...bindings, [id]: keys } });
    },
    setMouse: (patch) => save({ mouse: { ...get().mouse, ...patch } }),
    resetMouse: () => save({ mouse: DEFAULT_MOUSE }),
    setDialogOpen: (dialogOpen) => set({ dialogOpen }),
  };
});

/** The first key of a command as shown on screen, or null if it has none. */
export function useKeyText(id: CommandId): string | null {
  const key = useInput((s) => s.bindings[id][0]);
  return key ? formatKey(key) : null;
}

/** The held keys of a mouse setting as written in help texts, e.g. "Shift / ⌘". */
export function useHeldText(setting: 'boxSelect' | 'addToSelection' | 'constrain' | 'followToggle'): string {
  return heldText(useInput((s) => s.mouse[setting]));
}

const typing = (t: EventTarget | null) =>
  t instanceof HTMLElement && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);

/**
 * Runs the handlers when their command's keys are pressed, outside text
 * fields (except the commands in `inFields`) and while the settings dialog
 * is closed.
 */
export function useCommandKeys(handlers: Partial<Record<CommandId, () => void>>, inFields: CommandId[] = []) {
  const onKey = useEffectEvent((e: KeyboardEvent) => {
    if (e.isComposing || useInput.getState().dialogOpen) return;
    const { bindings } = useInput.getState();
    const inField = typing(e.target);
    for (const [id, run] of Object.entries(handlers) as [CommandId, () => void][]) {
      if (inField && !inFields.includes(id)) continue;
      if (!bindings[id].some((k) => matches(k, e))) continue;
      e.preventDefault();
      if (inField) (e.target as HTMLElement).blur();
      run();
    }
  });
  useEffect(() => {
    const listener = (e: KeyboardEvent) => onKey(e);
    document.documentElement.addEventListener('keydown', listener);
    return () => document.documentElement.removeEventListener('keydown', listener);
  }, []);
}
