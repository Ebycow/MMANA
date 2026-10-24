// The dialog that chooses where the wire list's 追加 button puts a new wire.
import { Group, Modal, Radio, Stack, Text, TextInput } from '@mantine/core';
import { useEffect, useState } from 'react';
import { lengthDigits, lengthFromDisplay, lengthToDisplay, lengthUnit, lengthWavelength, type Units } from '../domain/elements';
import { evaluate } from '../domain/expr';
import { wavelength } from '../domain/project';
import type { NewWireMode } from '../domain/wireEdit';
import { useDisplay } from '../state/display';
import { useNewWire } from '../state/newWire';
import { useApp } from '../state/store';
import { format } from './Cells';

export const NEW_WIRE_LABELS: Record<NewWireMode, string> = {
  zero: '原点に長さ 0',
  continue: '最後のワイヤーの終点につなげる',
  offset: '最後のワイヤーをずらした位置',
  fixed: '指定した座標',
};

const DESCRIPTIONS: Record<NewWireMode, string> = {
  zero: '座標はあとから表に入力します（MMANA と同じ）',
  continue: 'Y 方向に 1 m',
  offset: '形と向きはそのまま、下の量だけ平行に移します',
  fixed: '下の始点・終点',
};

export function NewWireSettings() {
  const s = useNewWire();
  const lambda = wavelength(useApp((st) => st.project.antenna.frequencyMhz));
  const display = useDisplay();
  // the wire table's units
  const units: Units = { wavelengths: display.wavelengths && lambda > 0, millimetres: display.millimetres, lambda };
  const lu = lengthUnit(units);
  const point = (key: 'offset' | 'start' | 'end', labels: string[], disabled: boolean) => (
    <Group grow gap="xs" wrap="nowrap">
      {(['x', 'y', 'z'] as const).map((a, k) => (
        <LengthInput key={a} label={labels[k]} value={s[key][a]} units={units} disabled={disabled} onCommit={(v) => s.set({ [key]: { ...s[key], [a]: v } })} />
      ))}
    </Group>
  );
  return (
    <Modal opened={s.dialogOpen} onClose={() => s.setDialogOpen(false)} title="ワイヤーの追加" size="md">
      <Stack gap="sm">
        <Text size="xs" c="dimmed">
          ワイヤー表の「ワイヤーを追加」で作るワイヤーの位置です。半径と SEG はどの場合も最後のワイヤーと同じにします。最後のワイヤーがないときは、つなげる・ずらすの代わりに原点から Y 方向に 1 m のワイヤーを追加します。
        </Text>
        <Radio.Group value={s.mode} onChange={(v) => s.set({ mode: v as NewWireMode })}>
          <Stack gap="xs">
            {(Object.keys(NEW_WIRE_LABELS) as NewWireMode[]).map((m) => (
              <Radio key={m} value={m} label={NEW_WIRE_LABELS[m]} description={DESCRIPTIONS[m]} />
            ))}
          </Stack>
        </Radio.Group>
        <Text size="sm" fw={500}>
          ずらす量（{lu}）
        </Text>
        {point('offset', ['ΔX', 'ΔY', 'ΔZ'], s.mode !== 'offset')}
        <Text size="sm" fw={500}>
          指定した座標（{lu}）
        </Text>
        {point('start', ['X1', 'Y1', 'Z1'], s.mode !== 'fixed')}
        {point('end', ['X2', 'Y2', 'Z2'], s.mode !== 'fixed')}
        <Text size="xs" c="dimmed">
          単位はワイヤー表と同じです。R（波長）や四則演算も使えます。この設定はドキュメントには含まれません。
        </Text>
      </Stack>
    </Modal>
  );
}

/** A length in the wire table's units; takes expressions like its cells (R = wavelength). */
function LengthInput({ label, value, units, disabled, onCommit }: { label: string; value: number; units: Units; disabled: boolean; onCommit: (m: number) => void }) {
  const shown = format(lengthToDisplay(value, units), lengthDigits(units));
  const [text, setText] = useState(shown);
  const [bad, setBad] = useState(false);
  useEffect(() => {
    setText(shown);
    setBad(false);
  }, [shown]);
  const commit = () => {
    if (text === shown) return setBad(false);
    const v = evaluate(text, { wavelength: lengthWavelength(units) });
    if (v == null) return setBad(true);
    setBad(false);
    onCommit(lengthFromDisplay(v, units));
    setText(format(v, lengthDigits(units)));
  };
  return (
    <TextInput
      size="xs"
      label={label}
      value={text}
      disabled={disabled}
      error={bad}
      classNames={{ input: 'num' }}
      onChange={(e) => setText(e.currentTarget.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
    />
  );
}
