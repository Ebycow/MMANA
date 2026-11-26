// The patterns across the band: the パターン page of MMANA's 周波数特性
// dialog. The cuts of up to five sweep frequencies on one grid, normalised to
// the highest gain among them.
import { Checkbox, Group, ScrollArea, SegmentedControl, Stack, Table, Text, Tooltip } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';
import type { PatternCut, SweepPoint } from '../api/types';
import { PolarCurves, PolarScaleSwitch, type PolarCurve } from '../charts/PolarPlot';
import { PATTERN_SLOTS, SLOT_COLORS, initialPatterns, togglePattern, type ShownPatterns } from '../domain/sweep';

/** 表示する偏波: 合算, 垂直, 水平, 重畳 (vertical and horizontal together). */
type Polarisation = 'total' | 'verticalPol' | 'horizontalPol' | 'both';

const POLARISATIONS: { value: Polarisation; label: string }[] = [
  { value: 'total', label: '合成' },
  { value: 'verticalPol', label: '垂直偏波' },
  { value: 'horizontalPol', label: '水平偏波' },
  { value: 'both', label: '垂直と水平' },
];

const mhz = (f: number) => `${f.toFixed(3)} MHz`;

function curvesOf(points: SweepPoint[], shown: ShownPatterns, pol: Polarisation, cut: (p: SweepPoint) => PatternCut): PolarCurve[] {
  return points.flatMap((p) => {
    const slot = shown.get(p.frequencyMhz);
    if (slot == null || !p.patterns) return [];
    const c = cut(p);
    const color = SLOT_COLORS[slot];
    if (pol === 'both') {
      return [
        { key: `${p.frequencyMhz}v`, label: `${mhz(p.frequencyMhz)} 垂直`, values: c.verticalPol, color, width: 1.5 },
        { key: `${p.frequencyMhz}h`, label: `${mhz(p.frequencyMhz)} 水平`, values: c.horizontalPol, color, dash: '5 3', width: 1.5 },
      ];
    }
    return [{ key: String(p.frequencyMhz), label: mhz(p.frequencyMhz), values: c[pol], color, width: 2 }];
  });
}

export function SweepPatterns({ data, opacity }: { data: SweepPoint[]; opacity: number }) {
  const frequencies = useMemo(() => data.map((p) => p.frequencyMhz), [data]);
  const key = frequencies.join(',');
  const [shown, setShown] = useState<ShownPatterns>(() => initialPatterns(frequencies));
  const [pol, setPol] = useState<Polarisation>('total');
  // a new sweep starts from its outline; points still arriving keep the choice
  useEffect(() => {
    setShown((s) => {
      const kept = new Map([...s].filter(([f]) => frequencies.includes(f)));
      return kept.size > 0 ? kept : initialPatterns(frequencies);
    });
  }, [key]);

  const visible = data.filter((p) => shown.has(p.frequencyMhz) && p.patterns);
  const maxDb = visible.length ? Math.max(...visible.map((p) => p.gainDbi)) : 0;
  const elevations = [...new Set(visible.map((p) => p.patterns!.horizontalElevationDeg.toFixed(1)))];
  const full = shown.size >= PATTERN_SLOTS;

  return (
    <Stack gap={4} style={{ flex: 1, minHeight: 0, opacity }}>
      <Group justify="space-between" gap="xs">
        <SegmentedControl size="xs" value={pol} onChange={(v) => setPol(v as Polarisation)} data={POLARISATIONS} aria-label="表示する偏波" />
        <Group gap="xs">
          {visible.length > 0 && (
            <Text size="xs" c="dimmed" className="num">
              表示中の最大利得 {maxDb.toFixed(2)} dBi を 0 dB として表示
            </Text>
          )}
          <PolarScaleSwitch />
        </Group>
      </Group>
      <div style={{ display: 'flex', gap: 12, flex: 1, minHeight: 240 }}>
        <ScrollArea w={272} style={{ flexShrink: 0 }} type="auto" offsetScrollbars>
          <Table fz="xs" className="num" verticalSpacing={2} horizontalSpacing={6} stickyHeader>
            <Table.Thead>
              <Table.Tr>
                <Table.Th />
                <Table.Th ta="right">周波数 (MHz)</Table.Th>
                <Table.Th ta="right">利得 (dBi)</Table.Th>
                <Table.Th ta="right">F/B (dB)</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {data.map((p) => {
                const slot = shown.get(p.frequencyMhz);
                const on = slot != null;
                const box = (
                  <Checkbox
                    size="xs"
                    checked={on}
                    disabled={(!on && full) || !p.patterns}
                    onChange={() => setShown((s) => togglePattern(s, p.frequencyMhz))}
                    aria-label={`${mhz(p.frequencyMhz)} の指向性を表示`}
                    color={on ? SLOT_COLORS[slot] : undefined}
                  />
                );
                return (
                  <Table.Tr key={p.frequencyMhz}>
                    <Table.Td>{!on && full ? <Tooltip label={`同時に表示できるのは ${PATTERN_SLOTS} 周波数までです`}>{box}</Tooltip> : box}</Table.Td>
                    <Table.Td ta="right">{p.frequencyMhz.toFixed(4)}</Table.Td>
                    <Table.Td ta="right">{p.gainDbi.toFixed(2)}</Table.Td>
                    <Table.Td ta="right">{p.fbDb.toFixed(2)}</Table.Td>
                  </Table.Tr>
                );
              })}
            </Table.Tbody>
          </Table>
        </ScrollArea>
        {visible.length === 0 ? (
          <Text size="sm" c="dimmed" m="auto">
            左の表で周波数を選ぶと、その周波数の指向性を重ねて表示します（{PATTERN_SLOTS} 周波数まで）。
          </Text>
        ) : (
          <>
            <PolarCurves
              kind="horizontal"
              stepDeg={visible[0].patterns!.horizontal.stepDeg}
              curves={curvesOf(data, shown, pol, (p) => p.patterns!.horizontal)}
              maxDb={maxDb}
              title="水平面"
              subtitle={elevations.length === 1 ? `仰角 ${elevations[0]}°` : '各周波数の打ち上げ角'}
            />
            <PolarCurves
              kind="vertical"
              stepDeg={visible[0].patterns!.vertical.stepDeg}
              curves={curvesOf(data, shown, pol, (p) => p.patterns!.vertical)}
              maxDb={maxDb}
              title="垂直面"
              subtitle="X-Z 面（右が前方）"
            />
          </>
        )}
      </div>
    </Stack>
  );
}
