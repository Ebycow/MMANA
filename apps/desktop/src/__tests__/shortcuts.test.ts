import { beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_BINDINGS, DEFAULT_MOUSE, canonical, conflicts, formatKey, fromEvent, isHeld, matches } from '../domain/shortcuts';
import { useInput } from '../state/input';

const key = (key: string, code: string, mods: { shift?: boolean; alt?: boolean; ctrl?: boolean; meta?: boolean } = {}) => ({
  key,
  code,
  shiftKey: !!mods.shift,
  altKey: !!mods.alt,
  ctrlKey: !!mods.ctrl,
  metaKey: !!mods.meta,
});

describe('key strings', () => {
  it('spells modifiers in one order and reads the platform key as mod', () => {
    expect(canonical('shift+mod+s', true)).toBe('mod+shift+S');
    expect(canonical('meta+Z', true)).toBe('mod+Z');
    expect(canonical('ctrl+Z', false)).toBe('mod+Z');
    expect(canonical('ctrl+Z', true)).toBe('ctrl+Z');
    expect(canonical('mod++', true)).toBe('mod++');
    expect(canonical('hyper+Z', true)).toBeNull();
  });

  it('matches ⌘ on a Mac and Ctrl elsewhere, exactly', () => {
    expect(matches('mod+S', key('s', 'KeyS', { meta: true }), true)).toBe(true);
    expect(matches('mod+S', key('s', 'KeyS', { ctrl: true }), true)).toBe(false);
    expect(matches('mod+S', key('s', 'KeyS', { ctrl: true }), false)).toBe(true);
    expect(matches('mod+S', key('S', 'KeyS', { meta: true, shift: true }), true)).toBe(false);
    expect(matches('mod+shift+S', key('S', 'KeyS', { meta: true, shift: true }), true)).toBe(true);
    expect(matches('V', key('v', 'KeyV'), true)).toBe(true);
    expect(matches('mod+Enter', key('Enter', 'Enter', { meta: true }), true)).toBe(true);
  });

  it('reads letters by position, so ⌥ on a Mac keeps them', () => {
    expect(fromEvent(key('ß', 'KeyS', { alt: true }), true)).toBe('alt+S');
    expect(matches('alt+S', key('ß', 'KeyS', { alt: true }), true)).toBe(true);
    expect(fromEvent(key('!', 'Digit1', { shift: true }), true)).toBe('shift+1');
  });

  it('takes a key press, but not modifiers alone or bare Esc / Tab / Enter', () => {
    expect(fromEvent(key('Meta', 'MetaLeft', { meta: true }), true)).toBeNull();
    expect(fromEvent(key('Escape', 'Escape'), true)).toBeNull();
    expect(fromEvent(key('Enter', 'Enter'), true)).toBeNull();
    expect(fromEvent(key('Enter', 'Enter', { meta: true }), true)).toBe('mod+Enter');
    expect(fromEvent(key(',', 'Comma', { meta: true }), true)).toBe('mod+,');
    expect(fromEvent(key('F5', 'F5'), false)).toBe('F5');
  });

  it('shows keys the platform way', () => {
    expect(formatKey('mod+shift+S', true)).toBe('⌘ ⇧ S');
    expect(formatKey('mod+shift+S', false)).toBe('Ctrl ⇧ S');
    expect(formatKey('mod+Enter', true)).toBe('⌘ ↵');
    expect(formatKey('alt+Backspace', false)).toBe('Alt ⌫');
  });

  it('finds the commands that already use a key', () => {
    expect(conflicts(DEFAULT_BINDINGS, 'meta+s', undefined, true)).toEqual(['save']);
    expect(conflicts(DEFAULT_BINDINGS, 'mod+S', 'save', true)).toEqual([]);
  });

  it('gives every command a different default key', () => {
    const all = Object.values(DEFAULT_BINDINGS).flat().map((k) => canonical(k, true));
    expect(new Set(all).size).toBe(all.length);
  });
});

describe('held keys', () => {
  it('reads mod as ⌘ on a Mac and Ctrl elsewhere', () => {
    const click = { shiftKey: false, altKey: false, ctrlKey: true, metaKey: false };
    expect(isHeld(['mod'], click, true)).toBe(false);
    expect(isHeld(['mod'], click, false)).toBe(true);
    expect(isHeld(['shift', 'alt'], { ...click, altKey: true }, true)).toBe(true);
    expect(isHeld([], { ...click, shiftKey: true }, true)).toBe(false);
  });
});

describe('input settings', () => {
  beforeEach(() => {
    useInput.getState().resetKeys();
    useInput.getState().resetMouse();
  });

  it('moves a key from the command that had it', () => {
    const taken = useInput.getState().addKey('openSample', 'mod+O');
    expect(taken).toEqual(['open']);
    expect(useInput.getState().bindings.open).toEqual([]);
    expect(useInput.getState().bindings.openSample).toEqual(['mod+O']);
  });

  it('does not add a key twice', () => {
    useInput.getState().addKey('save', 'mod+s');
    expect(useInput.getState().bindings.save).toEqual(['mod+S']);
  });

  it('resets a command without taking keys back from others', () => {
    const s = useInput.getState();
    s.addKey('selectAll', 'mod+S');
    s.removeKey('save', 'mod+S');
    s.resetKeys('save');
    expect(useInput.getState().bindings.save).toEqual([]);
    expect(useInput.getState().bindings.selectAll).toEqual(['mod+A', 'mod+S']);
    s.resetKeys();
    expect(useInput.getState().bindings).toEqual(DEFAULT_BINDINGS);
  });

  it('changes the mouse settings', () => {
    useInput.getState().setMouse({ buttons: { ...DEFAULT_MOUSE.buttons, left: 'pan' }, boxSelect: ['alt'] });
    expect(useInput.getState().mouse.buttons).toEqual({ left: 'pan', middle: 'zoom', right: 'pan' });
    expect(useInput.getState().mouse.boxSelect).toEqual(['alt']);
    useInput.getState().resetMouse();
    expect(useInput.getState().mouse).toEqual(DEFAULT_MOUSE);
  });
});
