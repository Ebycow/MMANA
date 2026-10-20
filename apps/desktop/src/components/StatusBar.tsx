import { Group, Text } from '@mantine/core';
import { transport } from '../api/transport';
import { useApp } from '../state/store';

export function StatusBar() {
  const wires = useApp((s) => s.project.antenna.wires.length);
  const selected = useApp((s) => s.selectedWire);
  const count = useApp((s) => s.selectedWires.length);
  const result = useApp((s) => s.result);
  const ms = useApp((s) => s.lastCalcMs);
  const calculating = useApp((s) => s.calculating);
  const engine = transport.kind === 'tauri' ? 'ローカルエンジン' : 'HTTP API';
  return (
    <Group h="100%" px="md" gap="lg" wrap="nowrap">
      <Text size="xs" c="dimmed">
        ワイヤー {wires} 本{count > 1 ? `（${count} 本を選択中）` : selected != null ? `（No.${selected + 1} を選択中）` : ''}
      </Text>
      {result && (
        <Text size="xs" c="dimmed" className="num">
          パルス {result.pulseCount}・セグメントワイヤー {result.segmentedWires.length}
        </Text>
      )}
      <div style={{ flex: 1 }} />
      <Text size="xs" c="dimmed" className="num">
        {calculating ? '計算中…' : ms != null ? `計算時間 ${(ms / 1000).toFixed(2)} 秒` : ''}
      </Text>
      <Text size="xs" c="dimmed">
        {engine}
      </Text>
    </Group>
  );
}
