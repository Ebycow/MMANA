import { Badge, Button, Group, NumberInput, SegmentedControl, Stack, Table, Text, Tooltip } from '@mantine/core';
import { IconAlertTriangle, IconPlayerPlay } from '@tabler/icons-react';
import { useState } from 'react';
import { LineChart } from '../charts/LineChart';
import { useApp } from '../state/store';

export function SweepPanel() {
  const sweep = useApp((s) => s.sweep);
  const setRange = useApp((s) => s.setSweepRange);
  const run = useApp((s) => s.runSweep);
  const designFreq = useApp((s) => s.project.antenna.frequencyMhz);
  const [hover, setHover] = useState<number | null>(null);
  const [view, setView] = useState<'chart' | 'table'>('chart');
  const data = sweep.data;
  const opacity = sweep.stale ? 0.55 : 1;
  const x = data?.map((p) => p.frequencyMhz) ?? [];

  return (
    <Stack gap="xs" p="sm" pt={6} h="100%" style={{ boxSizing: 'border-box' }}>
      <Group gap="xs" align="end" wrap="nowrap">
        <NumberInput label="開始 (MHz)" w={110} value={sweep.startMhz} decimalScale={4} onChange={(v) => typeof v === 'number' && setRange({ startMhz: v })} />
        <NumberInput label="終了 (MHz)" w={110} value={sweep.stopMhz} decimalScale={4} onChange={(v) => typeof v === 'number' && setRange({ stopMhz: v })} />
        <NumberInput label="点数" w={80} value={sweep.points} min={2} max={401} allowDecimal={false} onChange={(v) => typeof v === 'number' && setRange({ points: v })} />
        <Button leftSection={<IconPlayerPlay size={14} />} loading={sweep.running} onClick={run} disabled={!(sweep.stopMhz > sweep.startMhz)}>
          計算
        </Button>
        {data && sweep.stale && (
          <Tooltip label="周波数特性の計算後にアンテナや条件が変更されています">
            <Badge color="yellow" variant="light" mb={6} leftSection={<IconAlertTriangle size={12} />}>
              再計算が必要
            </Badge>
          </Tooltip>
        )}
        <div style={{ flex: 1 }} />
        {data && (
          <SegmentedControl
            data={[
              { value: 'chart', label: 'グラフ' },
              { value: 'table', label: '表' },
            ]}
            value={view}
            onChange={(v) => setView(v as 'chart' | 'table')}
          />
        )}
      </Group>
      {!data ? (
        <Text size="sm" c="dimmed" mt="md" ta="center">
          範囲を指定して「計算」を押すと、SWR・インピーダンス・利得・F/B 比の周波数特性を表示します（2° 分解能で計算）。
        </Text>
      ) : view === 'chart' ? (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gridTemplateRows: '1fr 1fr', gap: 12, flex: 1, minHeight: 280, opacity }}>
          <LineChart title="SWR" unit="" x={x} hover={hover} onHover={setHover} yMin={1} yMax={10} markerX={designFreq}
            series={[{ label: 'SWR', values: data.map((p) => p.swr), color: 'var(--series-1)' }]} />
          <LineChart title="インピーダンス" unit="Ω" x={x} hover={hover} onHover={setHover} markerX={designFreq}
            series={[
              { label: 'R', values: data.map((p) => p.impedance.re), color: 'var(--series-1)' },
              { label: 'jX', values: data.map((p) => p.impedance.im), color: 'var(--series-2)', dash: '5 3' },
            ]} />
          <LineChart title="利得" unit="dBi" x={x} hover={hover} onHover={setHover} markerX={designFreq}
            series={[{ label: '利得', values: data.map((p) => p.gainDbi), color: 'var(--series-1)' }]} />
          <LineChart title="F/B 比" unit="dB" x={x} hover={hover} onHover={setHover} markerX={designFreq}
            series={[{ label: 'F/B', values: data.map((p) => p.fbDb), color: 'var(--series-1)' }]} />
        </div>
      ) : (
        <Table striped fz="xs" className="num" stickyHeader style={{ opacity }}>
          <Table.Thead>
            <Table.Tr>
              <Table.Th ta="right">周波数 (MHz)</Table.Th>
              <Table.Th ta="right">R (Ω)</Table.Th>
              <Table.Th ta="right">jX (Ω)</Table.Th>
              <Table.Th ta="right">SWR</Table.Th>
              <Table.Th ta="right">利得 (dBi)</Table.Th>
              <Table.Th ta="right">F/B (dB)</Table.Th>
              <Table.Th ta="right">打ち上げ角 (°)</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {data.map((p) => (
              <Table.Tr key={p.frequencyMhz}>
                <Table.Td ta="right">{p.frequencyMhz.toFixed(4)}</Table.Td>
                <Table.Td ta="right">{p.impedance.re.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.impedance.im.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.swr >= 99999 ? '∞' : p.swr.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.gainDbi.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.fbDb.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.elevationDeg.toFixed(1)}</Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      )}
    </Stack>
  );
}
