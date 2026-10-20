// Form fields that commit on blur / Enter, so each edit is one undo step and
// the calculation is not marked stale on every keystroke.
import { NumberInput, TextInput, type NumberInputProps, type TextInputProps } from '@mantine/core';
import { useEffect, useState } from 'react';

type FieldNumberProps = Omit<NumberInputProps, 'value' | 'onChange'> & {
  value: number | null | undefined;
  onCommit: (v: number) => void;
};

export function FieldNumber({ value, onCommit, ...rest }: FieldNumberProps) {
  const [v, setV] = useState<string | number>(value ?? '');
  useEffect(() => setV(value ?? ''), [value]);
  const commit = () => {
    const n = typeof v === 'number' ? v : Number(v);
    if (v === '' || !Number.isFinite(n)) return setV(value ?? '');
    if (n !== value) onCommit(n);
  };
  return (
    <NumberInput
      {...rest}
      value={v}
      onChange={setV}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
      classNames={{ input: 'num' }}
    />
  );
}

type FieldTextProps = Omit<TextInputProps, 'value' | 'onChange'> & {
  value: string;
  onCommit: (v: string) => void;
};

export function FieldText({ value, onCommit, ...rest }: FieldTextProps) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  const commit = () => v !== value && onCommit(v);
  return (
    <TextInput
      {...rest}
      value={v}
      onChange={(e) => setV(e.currentTarget.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
    />
  );
}
