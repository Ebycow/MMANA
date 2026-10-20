// Spreadsheet-style cells: show a formatted value, edit the raw text,
// commit on Enter / blur, revert on Escape. Numeric cells accept MMANA-style
// expressions (R = wavelength, I = inch, F = foot).
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { evaluate } from '../domain/expr';

interface CellBaseProps {
  ariaLabel: string;
  align?: 'left' | 'right';
}

/** Enter commits and moves focus to the same column one row down. */
function moveDown(input: HTMLInputElement) {
  const td = input.closest('td');
  const tr = td?.parentElement;
  if (!td || !tr) return;
  const col = Array.from(tr.children).indexOf(td);
  const next = tr.nextElementSibling?.children[col]?.querySelector('input');
  (next as HTMLInputElement | null)?.focus();
}

interface NumCellProps extends CellBaseProps {
  value: number;
  onCommit: (v: number) => void;
  /** Decimal places shown when not editing. */
  digits?: number;
  /** Displayed value = stored value × scale (e.g. 1000 for mm). */
  scale?: number;
  wavelength?: number;
  validate?: (v: number) => boolean;
  /** Shows the value but does not accept edits. */
  readOnly?: boolean;
}

export function format(value: number, digits: number): string {
  if (!Number.isFinite(value)) return '';
  return Number(value.toFixed(digits)).toString();
}

export function NumCell({ value, onCommit, digits = 4, scale = 1, wavelength = 1, validate, readOnly, ariaLabel, align }: NumCellProps) {
  const shown = format(value * scale, digits);
  const [text, setText] = useState(shown);
  const [invalid, setInvalid] = useState(false);
  const editing = useRef(false);

  useEffect(() => {
    if (!editing.current) setText(shown);
  }, [shown]);

  const commit = () => {
    editing.current = false;
    if (text === shown) return setInvalid(false);
    const v = evaluate(text, { wavelength: wavelength * scale });
    if (v == null || (validate && !validate(v / scale))) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    if (v / scale !== value) onCommit(v / scale);
    setText(format(v, digits));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      commit();
      moveDown(e.currentTarget);
    } else if (e.key === 'Escape') {
      editing.current = false;
      setText(shown);
      setInvalid(false);
      e.currentTarget.blur();
    }
  };

  return (
    <input
      className={`cell-input${invalid ? ' invalid' : ''}${align === 'left' ? ' left' : ''}`}
      aria-label={ariaLabel}
      aria-invalid={invalid}
      value={text}
      readOnly={readOnly}
      spellCheck={false}
      onFocus={(e) => {
        editing.current = true;
        e.currentTarget.select();
      }}
      onChange={(e) => setText(e.currentTarget.value)}
      onBlur={commit}
      onKeyDown={onKeyDown}
    />
  );
}

interface TextCellProps extends CellBaseProps {
  value: string;
  onCommit: (v: string) => void;
  /** Returns false to reject the text. */
  validate?: (v: string) => boolean;
  placeholder?: string;
}

export function TextCell({ value, onCommit, validate, ariaLabel, align = 'left', placeholder }: TextCellProps) {
  const [text, setText] = useState(value);
  const [invalid, setInvalid] = useState(false);
  const editing = useRef(false);

  useEffect(() => {
    if (!editing.current) setText(value);
  }, [value]);

  const commit = () => {
    editing.current = false;
    if (text === value) return setInvalid(false);
    if (validate && !validate(text)) return setInvalid(true);
    setInvalid(false);
    onCommit(text);
  };

  return (
    <input
      className={`cell-input${invalid ? ' invalid' : ''}${align === 'left' ? ' left' : ''}`}
      aria-label={ariaLabel}
      aria-invalid={invalid}
      value={text}
      placeholder={placeholder}
      spellCheck={false}
      onFocus={() => (editing.current = true)}
      onChange={(e) => setText(e.currentTarget.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          commit();
          moveDown(e.currentTarget);
        } else if (e.key === 'Escape') {
          editing.current = false;
          setText(value);
          setInvalid(false);
          e.currentTarget.blur();
        }
      }}
    />
  );
}
