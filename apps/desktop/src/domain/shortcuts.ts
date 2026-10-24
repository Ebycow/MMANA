// Keyboard shortcuts and mouse modifiers the viewer can reassign: the commands
// that take a key, the key strings ("mod+shift+S", where mod is ⌘ on a Mac
// and Ctrl elsewhere), reading one from a key press, matching and showing it.
import type { PlaneMode, Tool } from '../state/editor';

export type CommandId =
  | 'newFile'
  | 'open'
  | 'openSample'
  | 'save'
  | 'saveAs'
  | 'print'
  | 'undo'
  | 'redo'
  | 'copy'
  | 'cut'
  | 'paste'
  | 'duplicate'
  | 'selectAll'
  | 'delete'
  | 'group'
  | 'ungroup'
  | 'calculate'
  | 'optimize'
  | 'settings'
  | 'drawPlane'
  | `drawPlane.${PlaneMode}`
  | `tool.${Tool}`;

export interface Command {
  id: CommandId;
  label: string;
  group: string;
  keys: string[];
}

export const COMMANDS: Command[] = [
  { id: 'newFile', label: '新規作成', group: 'ファイル', keys: [] },
  { id: 'open', label: '開く', group: 'ファイル', keys: ['mod+O'] },
  { id: 'openSample', label: 'サンプルを開く', group: 'ファイル', keys: [] },
  { id: 'save', label: '保存', group: 'ファイル', keys: ['mod+S'] },
  { id: 'saveAs', label: '名前を付けて保存', group: 'ファイル', keys: ['mod+shift+S'] },
  { id: 'print', label: '印刷', group: 'ファイル', keys: ['mod+P'] },
  { id: 'undo', label: '元に戻す', group: '編集', keys: ['mod+Z'] },
  { id: 'redo', label: 'やり直す', group: '編集', keys: ['mod+shift+Z'] },
  { id: 'copy', label: 'コピー', group: '編集', keys: ['mod+C'] },
  { id: 'cut', label: '切り取り', group: '編集', keys: ['mod+X'] },
  { id: 'paste', label: '貼り付け', group: '編集', keys: ['mod+V'] },
  { id: 'duplicate', label: '複製', group: '編集', keys: ['mod+D'] },
  { id: 'selectAll', label: 'すべて選択', group: '編集', keys: ['mod+A'] },
  { id: 'delete', label: '選択したワイヤーを削除', group: '編集', keys: ['Delete', 'Backspace'] },
  { id: 'group', label: '選択したワイヤーをグループにまとめる', group: '編集', keys: ['mod+G'] },
  { id: 'ungroup', label: '選択したワイヤーのグループを解除', group: '編集', keys: ['mod+shift+G'] },
  { id: 'calculate', label: '計算', group: '計算', keys: ['mod+Enter'] },
  { id: 'optimize', label: '最適化', group: '計算', keys: [] },
  { id: 'tool.select', label: '選択', group: '3D ビューのツール', keys: ['V'] },
  { id: 'tool.move', label: '移動', group: '3D ビューのツール', keys: ['G'] },
  { id: 'tool.rotate', label: '回転', group: '3D ビューのツール', keys: ['R'] },
  { id: 'tool.wire', label: 'ワイヤーを描く', group: '3D ビューのツール', keys: ['W'] },
  { id: 'tool.rect', label: '四角ループ', group: '3D ビューのツール', keys: ['Q'] },
  { id: 'tool.polygon', label: '多角形ループ', group: '3D ビューのツール', keys: ['O'] },
  { id: 'tool.feed', label: '給電点・負荷を移動', group: '3D ビューのツール', keys: ['F'] },
  { id: 'drawPlane', label: '作図面を切り替え（自動 → XY → XZ → YZ）', group: '3D ビューのツール', keys: ['P'] },
  { id: 'drawPlane.auto', label: '作図面を自動に', group: '3D ビューのツール', keys: [] },
  { id: 'drawPlane.xy', label: '作図面を XY に', group: '3D ビューのツール', keys: [] },
  { id: 'drawPlane.xz', label: '作図面を XZ に', group: '3D ビューのツール', keys: [] },
  { id: 'drawPlane.yz', label: '作図面を YZ に', group: '3D ビューのツール', keys: [] },
  { id: 'settings', label: 'キーボードとマウスの設定', group: 'その他', keys: ['mod+,'] },
];

export type Bindings = Record<CommandId, string[]>;

export const DEFAULT_BINDINGS = Object.fromEntries(COMMANDS.map((c) => [c.id, c.keys])) as Bindings;

export const IS_MAC = typeof navigator !== 'undefined' && /mac/i.test(navigator.platform);

/** The modifiers in a key string, in the order they are written. */
const MODIFIERS = ['mod', 'ctrl', 'meta', 'alt', 'shift'] as const;
type Modifier = (typeof MODIFIERS)[number];

interface Parsed {
  mods: Set<Modifier>;
  key: string;
}

/**
 * Reads a key string. The platform's command key is `mod` (⌘ on a Mac, Ctrl
 * elsewhere), so `ctrl` on Windows and `meta` on a Mac mean `mod`.
 */
function parse(binding: string, mac: boolean): Parsed | null {
  const parts = binding.split('+');
  // the "+" key itself leaves two empty parts at the end ("mod++")
  const key = parts.at(-1) === '' ? '+' : parts.pop()!;
  if (key === '+') parts.splice(-2);
  const mods = new Set<Modifier>();
  for (const p of parts) {
    let m = p.trim().toLowerCase() as Modifier;
    if (!MODIFIERS.includes(m)) return null;
    if (m === (mac ? 'meta' : 'ctrl')) m = 'mod';
    mods.add(m);
  }
  return key ? { mods, key: normalizeKey(key) } : null;
}

function normalizeKey(key: string): string {
  if (key === 'Esc') return 'Escape';
  if (key === 'Spacebar') key = ' ';
  return key.length === 1 ? key.toUpperCase() : key[0].toUpperCase() + key.slice(1);
}

const write = ({ mods, key }: Parsed) => [...MODIFIERS.filter((m) => mods.has(m)), key].join('+');

/** The key string in its usual spelling, or null if it is not one. */
export function canonical(binding: string, mac = IS_MAC): string | null {
  const p = parse(binding, mac);
  return p && write(p);
}

/** The key of a key press: letters and digits by position, so ⌥ or ⇧ on a Mac keeps them. */
function eventKey(e: Pick<KeyboardEvent, 'key' | 'code'>): string {
  const m = /^(?:Key([A-Z])|Digit([0-9]))$/.exec(e.code);
  if (m) return m[1] ?? m[2];
  return normalizeKey(e.key);
}

type KeyEvent = Pick<KeyboardEvent, 'key' | 'code' | 'shiftKey' | 'altKey' | 'ctrlKey' | 'metaKey'>;

/** Whether a key press is the shortcut. */
export function matches(binding: string, e: KeyEvent, mac = IS_MAC): boolean {
  const p = parse(binding, mac);
  if (!p) return false;
  const modKey = mac ? e.metaKey : e.ctrlKey;
  const other = mac ? e.ctrlKey : e.metaKey;
  return (
    p.mods.has('mod') === modKey &&
    p.mods.has(mac ? 'ctrl' : 'meta') === other &&
    p.mods.has('alt') === e.altKey &&
    p.mods.has('shift') === e.shiftKey &&
    p.key === eventKey(e)
  );
}

/** Keys that keep their own meaning and cannot be assigned on their own. */
const RESERVED = new Set(['Escape', 'Tab', 'Enter']);
const MODIFIER_KEYS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'OS', 'CapsLock', 'Fn', 'Dead', 'Process', 'Unidentified']);

/**
 * The key string for a key press while assigning one: null while only
 * modifiers are held, or for Esc, Tab and Enter pressed without ⌘ / Ctrl / ⌥.
 */
export function fromEvent(e: KeyEvent, mac = IS_MAC): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const mods = new Set<Modifier>();
  if (mac ? e.metaKey : e.ctrlKey) mods.add('mod');
  if (mac ? e.ctrlKey : e.metaKey) mods.add(mac ? 'ctrl' : 'meta');
  if (e.altKey) mods.add('alt');
  if (e.shiftKey) mods.add('shift');
  const key = eventKey(e);
  if (RESERVED.has(key) && !mods.has('mod') && !mods.has('ctrl') && !mods.has('meta') && !mods.has('alt')) return null;
  return write({ mods, key });
}

const KEY_LABEL: Record<string, string> = {
  Enter: '↵',
  Backspace: '⌫',
  Delete: 'Delete',
  Escape: 'Esc',
  ' ': 'Space',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
};

/** The key string as shown on screen, e.g. "⌘ ⇧ S" or "Ctrl ⇧ S". */
export function formatKey(binding: string, mac = IS_MAC): string {
  const p = parse(binding, mac);
  if (!p) return binding;
  const label: Record<Modifier, string> = mac
    ? { mod: '⌘', ctrl: '⌃', meta: '⌘', alt: '⌥', shift: '⇧' }
    : { mod: 'Ctrl', ctrl: 'Ctrl', meta: 'Win', alt: 'Alt', shift: '⇧' };
  return [...MODIFIERS.filter((m) => p.mods.has(m)).map((m) => label[m]), KEY_LABEL[p.key] ?? p.key].join(' ');
}

/** The commands other than `except` that already use the key. */
export function conflicts(bindings: Bindings, binding: string, except?: CommandId, mac = IS_MAC): CommandId[] {
  const key = canonical(binding, mac);
  return COMMANDS.filter((c) => c.id !== except && bindings[c.id].some((k) => canonical(k, mac) === key)).map((c) => c.id);
}

// ---- mouse ----

/** A held key that changes what a click or drag in the 3D view does. */
export type HeldKey = 'shift' | 'alt' | 'mod';

export function heldLabel(k: HeldKey, mac = IS_MAC): string {
  return k === 'shift' ? 'Shift' : k === 'alt' ? (mac ? '⌥' : 'Alt') : mac ? '⌘' : 'Ctrl';
}

/** Whether any of the keys is held. */
export function isHeld(keys: HeldKey[], e: Pick<MouseEvent, 'shiftKey' | 'altKey' | 'ctrlKey' | 'metaKey'>, mac = IS_MAC): boolean {
  return keys.some((k) => (k === 'shift' ? e.shiftKey : k === 'alt' ? e.altKey : mac ? e.metaKey : e.ctrlKey));
}

/** The keys as they are written in help texts, e.g. "Shift / ⌘". */
export const heldText = (keys: HeldKey[], mac = IS_MAC) => (keys.length ? keys.map((k) => heldLabel(k, mac)).join(' / ') : '（なし）');

export type CameraAction = 'rotate' | 'pan' | 'zoom' | 'none';
export type MouseButtonName = 'left' | 'middle' | 'right';

export interface MouseSettings {
  /** What dragging with each button does to the camera on empty space. */
  buttons: Record<MouseButtonName, CameraAction>;
  /** Wheel up (and dragging up with the zoom button) zooms out instead of in. */
  reverseZoom: boolean;
  zoomToCursor: boolean;
  zoomSpeed: number;
  rotateSpeed: number;
  /** Held while dragging on empty space: box selection. */
  boxSelect: HeldKey[];
  /** Held while clicking a wire: add it to or take it from the selection. */
  addToSelection: HeldKey[];
  /** Held while dragging or drawing: keep to one axis, 15° steps, no snapping to ends. */
  constrain: HeldKey[];
  /** Held while dragging: the opposite of 「つながりを保つ」. */
  followToggle: HeldKey[];
}

export const DEFAULT_MOUSE: MouseSettings = {
  buttons: { left: 'rotate', middle: 'zoom', right: 'pan' },
  reverseZoom: false,
  zoomToCursor: false,
  zoomSpeed: 1,
  rotateSpeed: 1,
  boxSelect: ['shift'],
  addToSelection: ['shift', 'mod'],
  constrain: ['shift'],
  followToggle: ['alt'],
};
