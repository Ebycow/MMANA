import { NumberInput, SegmentedControl, Select, SimpleGrid, Stack, Switch, Text, Textarea, Title } from '@mantine/core';
import { useEffect, useState } from 'react';
import type { StackDirection } from '../api/types';
import { useApp } from '../state/store';
import { FieldNumber } from './Fields';

export function AdvancedForm() {
  const ant = useApp((s) => s.project.antenna);
  const options = useApp((s) => s.options);
  const setOptions = useApp((s) => s.setOptions);
  const edit = useApp((s) => s.edit);
  const seg = ant.autoSegmentation;
  const st = ant.stack;

  return (
    <Stack gap="lg" p="sm" maw={640}>
      <section>
        <Title order={6} mb={4}>
          自動分割
        </Title>
        <Text size="xs" c="dimmed" mb="xs">
          SEG が 0 以下のワイヤーに適用されます。テーパでは端を λ/DM1、中央を λ/DM2 の長さで分割します。
        </Text>
        <SimpleGrid cols={4}>
          <FieldNumber label="DM1" value={seg.dm1} min={1} allowDecimal={false} onCommit={(v) => edit((p) => void (p.antenna.autoSegmentation.dm1 = v))} />
          <FieldNumber label="DM2" value={seg.dm2} min={1} allowDecimal={false} onCommit={(v) => edit((p) => void (p.antenna.autoSegmentation.dm2 = v))} />
          <FieldNumber
            label="SC（拡大率）"
            value={seg.sc}
            min={1.01}
            max={3}
            step={0.1}
            decimalScale={2}
            onCommit={(v) => edit((p) => void (p.antenna.autoSegmentation.sc = v))}
          />
          <FieldNumber
            label="EC（端の分割数）"
            value={seg.ec}
            min={1}
            max={127}
            allowDecimal={false}
            onCommit={(v) => edit((p) => void (p.antenna.autoSegmentation.ec = v))}
          />
        </SimpleGrid>
      </section>

      <section>
        <Title order={6} mb={4}>
          スタック
        </Title>
        <Text size="xs" c="dimmed" mb="xs">
          アンテナ全体を横（Y 方向）と縦（Z 方向）に並べ、各給電点の電圧を等分します。
        </Text>
        <SimpleGrid cols={2}>
          <FieldNumber label="横の数" value={st.horizontal} min={1} allowDecimal={false} onCommit={(v) => edit((p) => void (p.antenna.stack.horizontal = v))} />
          <FieldNumber label="横の間隔 (m)" value={st.horizontalSpacing} decimalScale={3} onCommit={(v) => edit((p) => void (p.antenna.stack.horizontalSpacing = v))} />
          <FieldNumber label="縦の数" value={st.vertical} min={1} allowDecimal={false} onCommit={(v) => edit((p) => void (p.antenna.stack.vertical = v))} />
          <FieldNumber label="縦の間隔 (m)" value={st.verticalSpacing} decimalScale={3} onCommit={(v) => edit((p) => void (p.antenna.stack.verticalSpacing = v))} />
        </SimpleGrid>
        <Select
          mt="sm"
          label="縦に並べる向き"
          data={[
            { value: 'centered', label: '上下に振り分け' },
            { value: 'up', label: '上向き' },
            { value: 'down', label: '下向き' },
          ]}
          value={st.direction}
          onChange={(v) => v && edit((p) => void (p.antenna.stack.direction = v as StackDirection))}
        />
      </section>

      <section>
        <Title order={6} mb="xs">
          計算オプション
        </Title>
        <SimpleGrid cols={2}>
          <div>
            <Text size="xs" fw={500} mb={4}>
              パターンの分解能
            </Text>
            <SegmentedControl
              fullWidth
              data={[
                { value: 'fine', label: '1°' },
                { value: 'coarse', label: '2°（高速）' },
              ]}
              value={options.patternResolution}
              onChange={(v) => setOptions({ patternResolution: v as 'fine' | 'coarse' })}
            />
          </div>
          <NumberInput
            label="パルス数の上限"
            value={options.maxPulses}
            min={1}
            max={8192}
            allowDecimal={false}
            onChange={(v) => typeof v === 'number' && setOptions({ maxPulses: v })}
          />
          <OptionalNumber
            label="水平パターンの仰角 (°)"
            placeholder="自動（最大放射の仰角）"
            value={options.horizontalCutElevationDeg}
            onChange={(v) => setOptions({ horizontalCutElevationDeg: v })}
          />
        </SimpleGrid>
        <Switch
          mt="sm"
          label="自由空間では水平パターンを仰角 0° で求める"
          checked={options.fixFreeSpaceElevation}
          onChange={(e) => setOptions({ fixFreeSpaceElevation: e.currentTarget.checked })}
        />
      </section>

      <section>
        <Title order={6} mb="xs">
          コメント
        </Title>
        <Comment value={ant.comment ?? ''} onCommit={(v) => edit((p) => void (p.antenna.comment = v))} />
      </section>
    </Stack>
  );
}

function OptionalNumber(props: { label: string; placeholder: string; value: number | null; onChange: (v: number | null) => void }) {
  return (
    <NumberInput
      label={props.label}
      placeholder={props.placeholder}
      value={props.value ?? ''}
      min={-90}
      max={90}
      onChange={(v) => props.onChange(typeof v === 'number' ? v : null)}
    />
  );
}

function Comment({ value, onCommit }: { value: string; onCommit: (v: string) => void }) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  return (
    <Textarea
      autosize
      minRows={4}
      maxRows={14}
      value={v}
      onChange={(e) => setV(e.currentTarget.value)}
      onBlur={() => v !== value && onCommit(v)}
      styles={{ input: { fontFamily: 'var(--mantine-font-family-monospace)', fontSize: 12 } }}
    />
  );
}
