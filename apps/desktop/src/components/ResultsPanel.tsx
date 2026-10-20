import { Alert, Badge, Button, Center, Group, Kbd, Paper, SimpleGrid, Stack, Table, Tabs, Text, ThemeIcon, Tooltip } from '@mantine/core';
import { IconAlertTriangle, IconChartDots3, IconCircuitGround, IconPlayerPlay, IconRadar2, IconWaveSine } from '@tabler/icons-react';
import type { CalcResult } from '../api/types';
import { PolarLegend, PolarPlot } from '../charts/PolarPlot';
import { useApp } from '../state/store';
import { Panel } from './Panel';
import { SweepPanel } from './SweepPanel';

export const fmtZ = (re: number, im: number) => `${re.toFixed(2)} ${im < 0 ? '−' : '+'} j${Math.abs(im).toFixed(2)}`;

function Kpi({ label, value, unit, hint }: { label: string; value: string; unit?: string; hint?: string }) {
  return (
    <Paper withBorder px="xs" py={6} radius="md" style={{ background: 'var(--chart-surface)' }} title={hint}>
      <Text size="xs" c="dimmed" truncate>
        {label}
      </Text>
      <Group gap={4} align="baseline" wrap="nowrap">
        <Text fw={650} size="md" className="num" style={{ whiteSpace: 'nowrap' }}>
          {value}
        </Text>
        {unit && (
          <Text size="xs" c="dimmed">
            {unit}
          </Text>
        )}
      </Group>
    </Paper>
  );
}

function Summary({ r }: { r: CalcResult }) {
  const ref = useApp((s) => s.project.environment.swrReference);
  return (
    <SimpleGrid cols={6} spacing={8} p="sm" pb={4}>
      <Kpi label="インピーダンス" value={fmtZ(r.impedance.re, r.impedance.im)} unit="Ω" />
      <Kpi
        label={`SWR（${ref.x === 0 ? ref.r : fmtZ(ref.r, ref.x)} Ω）`}
        value={r.swr >= 99999 ? '∞' : r.swr.toFixed(2)}
        hint={`基準インピーダンス ${fmtZ(ref.r, ref.x)} Ω`}
      />
      <Kpi label="利得" value={r.gainDbi.toFixed(2)} unit="dBi" hint={`${(r.gainDbi - 2.15).toFixed(2)} dBd`} />
      <Kpi label="F/B 比" value={r.fbDb.toFixed(2)} unit="dB" />
      <Kpi label="打ち上げ角" value={r.elevationDeg.toFixed(1)} unit="°" />
      <Kpi label="周波数" value={r.frequencyMhz.toFixed(3)} unit="MHz" hint={`λ = ${r.wavelengthM.toFixed(3)} m`} />
    </SimpleGrid>
  );
}

function Patterns({ r }: { r: CalcResult }) {
  const p = r.patterns;
  return (
    <Stack gap={4} p="sm" pt={4} style={{ height: '100%', boxSizing: 'border-box' }}>
      <Group justify="space-between">
        <PolarLegend />
        <Text size="xs" c="dimmed">
          最大利得 {r.gainDbi.toFixed(2)} dBi を 0 dB として表示
        </Text>
      </Group>
      <div style={{ display: 'flex', gap: 16, flex: 1, minHeight: 0 }}>
        <PolarPlot
          kind="horizontal"
          cut={p.horizontal}
          maxDb={r.gainDbi}
          title="水平面"
          subtitle={`仰角 ${p.horizontalElevationDeg.toFixed(1)}°`}
        />
        <PolarPlot kind="vertical" cut={p.vertical} maxDb={r.gainDbi} title="垂直面" subtitle="X-Z 面（右が前方）" />
      </div>
    </Stack>
  );
}

function Currents({ r }: { r: CalcResult }) {
  return (
    <Table striped highlightOnHover stickyHeader fz="xs" className="num" horizontalSpacing="sm" verticalSpacing={3}>
      <Table.Thead>
        <Table.Tr>
          <Table.Th>パルス</Table.Th>
          <Table.Th>ワイヤー</Table.Th>
          <Table.Th ta="right">X (m)</Table.Th>
          <Table.Th ta="right">Y (m)</Table.Th>
          <Table.Th ta="right">Z (m)</Table.Th>
          <Table.Th ta="right">|I| (mA)</Table.Th>
          <Table.Th ta="right">位相 (°)</Table.Th>
        </Table.Tr>
      </Table.Thead>
      <Table.Tbody>
        {r.currents.map((c) => (
          <Table.Tr key={c.pulse}>
            <Table.Td>{c.pulse}</Table.Td>
            <Table.Td>{c.wire}</Table.Td>
            <Table.Td ta="right">{c.position.x.toFixed(3)}</Table.Td>
            <Table.Td ta="right">{c.position.y.toFixed(3)}</Table.Td>
            <Table.Td ta="right">{c.position.z.toFixed(3)}</Table.Td>
            <Table.Td ta="right">{(Math.hypot(c.current.re, c.current.im) * 1000).toFixed(3)}</Table.Td>
            <Table.Td ta="right">{((Math.atan2(c.current.im, c.current.re) * 180) / Math.PI).toFixed(1)}</Table.Td>
          </Table.Tr>
        ))}
      </Table.Tbody>
    </Table>
  );
}

function Sources({ r }: { r: CalcResult }) {
  return (
    <Table striped fz="xs" className="num" horizontalSpacing="sm">
      <Table.Thead>
        <Table.Tr>
          <Table.Th>給電点</Table.Th>
          <Table.Th>パルス</Table.Th>
          <Table.Th ta="right">電圧 (V)</Table.Th>
          <Table.Th ta="right">電流 (mA)</Table.Th>
          <Table.Th ta="right">インピーダンス (Ω)</Table.Th>
          <Table.Th ta="right">SWR</Table.Th>
          <Table.Th ta="right">電力 (W)</Table.Th>
        </Table.Tr>
      </Table.Thead>
      <Table.Tbody>
        {r.sources.map((s, i) => (
          <Table.Tr key={i}>
            <Table.Td>{i + 1}</Table.Td>
            <Table.Td>{s.pulse}</Table.Td>
            <Table.Td ta="right">{fmtZ(s.voltage.re, s.voltage.im)}</Table.Td>
            <Table.Td ta="right">{fmtZ(s.current.re * 1000, s.current.im * 1000)}</Table.Td>
            <Table.Td ta="right">{fmtZ(s.impedance.re, s.impedance.im)}</Table.Td>
            <Table.Td ta="right">{s.swr >= 99999 ? '∞' : s.swr.toFixed(2)}</Table.Td>
            <Table.Td ta="right">{s.powerW.toExponential(3)}</Table.Td>
          </Table.Tr>
        ))}
      </Table.Tbody>
    </Table>
  );
}

function EmptyState() {
  const calculate = useApp((s) => s.calculate);
  const calculating = useApp((s) => s.calculating);
  return (
    <Center h="100%">
      <Stack align="center" gap="xs">
        <ThemeIcon size={44} radius="xl" variant="light">
          <IconRadar2 size={24} />
        </ThemeIcon>
        <Text fw={600}>まだ計算していません</Text>
        <Text size="sm" c="dimmed">
          インピーダンス・SWR・利得・指向性を計算します
        </Text>
        <Button leftSection={<IconPlayerPlay size={14} />} onClick={calculate} loading={calculating} mt={4}>
          計算する
        </Button>
        <Text size="xs" c="dimmed">
          <Kbd size="xs">⌘</Kbd> + <Kbd size="xs">Enter</Kbd> でも計算できます
        </Text>
      </Stack>
    </Center>
  );
}

export function ResultsPanel() {
  const r = useApp((s) => s.result);
  const stale = useApp((s) => s.resultStale);
  const error = useApp((s) => s.calcError);
  const dim = { opacity: stale ? 0.55 : 1 };

  return (
    <Panel
      title="計算結果"
      scroll={false}
      actions={
        stale ? (
          <Tooltip label="計算後にアンテナや条件が変更されています">
            <Badge color="yellow" variant="light" leftSection={<IconAlertTriangle size={12} />}>
              再計算が必要
            </Badge>
          </Tooltip>
        ) : undefined
      }
    >
      <div style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column' }}>
        {error && (
          <Alert color="red" variant="light" m="sm" mb={0} icon={<IconAlertTriangle size={16} />} title={error.title}>
            {error.detail}
          </Alert>
        )}
        {r ? (
          <>
            <div style={dim}>
              <Summary r={r} />
            </div>
            <Tabs defaultValue="pattern" keepMounted={false} style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
              <Tabs.List px="xs">
                <Tabs.Tab value="pattern" leftSection={<IconRadar2 size={14} />}>
                  指向性
                </Tabs.Tab>
                <Tabs.Tab value="sweep" leftSection={<IconWaveSine size={14} />}>
                  周波数特性
                </Tabs.Tab>
                <Tabs.Tab value="currents" leftSection={<IconChartDots3 size={14} />}>
                  電流
                  <Badge size="xs" variant="light" color="gray" ml={6}>
                    {r.pulseCount}
                  </Badge>
                </Tabs.Tab>
                <Tabs.Tab value="sources" leftSection={<IconCircuitGround size={14} />}>
                  給電点
                </Tabs.Tab>
              </Tabs.List>
              {/* the sweep has its own staleness; see SweepPanel */}
              <div style={{ flex: 1, minHeight: 0, overflow: 'auto' }}>
                <Tabs.Panel value="pattern" h="100%" style={dim}>
                  <Patterns r={r} />
                </Tabs.Panel>
                <Tabs.Panel value="sweep" h="100%">
                  <SweepPanel />
                </Tabs.Panel>
                <Tabs.Panel value="currents" style={dim}>
                  <Currents r={r} />
                </Tabs.Panel>
                <Tabs.Panel value="sources" style={dim}>
                  <Sources r={r} />
                </Tabs.Panel>
              </div>
            </Tabs>
            {r.warnings.length > 0 && (
              <Text size="xs" c="orange" px="sm" pb={6}>
                {r.warnings.join(' / ')}
              </Text>
            )}
          </>
        ) : (
          !error && <EmptyState />
        )}
      </div>
    </Panel>
  );
}
