import { Alert, Badge, Button, Checkbox, Group, Loader, NumberInput, SegmentedControl, Stack, Table, Text, Tooltip } from '@mantine/core';
import { IconAlertTriangle, IconPlayerPlay, IconPlayerStop, IconTarget } from '@tabler/icons-react';
import { useState } from 'react';
import { LineChart } from '../charts/LineChart';
import type { Lumped, ResonanceSearch, SweepPoint } from '../api/types';
import { sig } from '../domain/circuits';
import { swrBandwidth } from '../domain/print';
import type { MatchMode } from '../domain/sweep';
import { sweepAround, useApp, type SweepMatchState } from '../state/store';
import { FieldNumber } from './Fields';
import { SweepPatterns } from './SweepPatterns';

type View = 'chart' | 'patterns' | 'table';

const swrText = (v: number) => (v >= 99999 ? '∞' : v.toFixed(2));

export function SweepPanel() {
  const sweep = useApp((s) => s.sweep);
  const setRange = useApp((s) => s.setSweepRange);
  const run = useApp((s) => s.runSweep);
  const stop = useApp((s) => s.stopSweep);
  const designFreq = useApp((s) => s.project.antenna.frequencyMhz);
  const calcFreq = useApp((s) => s.options.frequencyMhz) ?? designFreq;
  const resonance = useApp((s) => s.resonance);
  const findResonance = useApp((s) => s.findResonance);
  const [hover, setHover] = useState<number | null>(null);
  const [view, setView] = useState<View>('chart');
  const data = sweep.data;
  const opacity = sweep.stale ? 0.55 : 1;
  const x = data?.map((p) => p.frequencyMhz) ?? [];
  const mode = sweep.matching.mode;
  const matched = matchedAt(data, sweep.match);

  return (
    <Stack gap="xs" p="sm" pt={6} h="100%" style={{ boxSizing: 'border-box' }}>
      <Group gap="xs" align="end" wrap="nowrap">
        <NumberInput label="開始 (MHz)" w={110} value={sweep.startMhz} decimalScale={4} onChange={(v) => typeof v === 'number' && setRange({ startMhz: v })} />
        <NumberInput label="終了 (MHz)" w={110} value={sweep.stopMhz} decimalScale={4} onChange={(v) => typeof v === 'number' && setRange({ stopMhz: v })} />
        <NumberInput label="点数" w={80} value={sweep.points} min={2} max={401} allowDecimal={false} onChange={(v) => typeof v === 'number' && setRange({ points: v })} />
        <Button leftSection={<IconPlayerPlay size={14} />} loading={sweep.running} onClick={run} disabled={!(sweep.stopMhz > sweep.startMhz)}>
          計算
        </Button>
        <Tooltip label={`計算周波数 ${calcFreq} MHz から、リアクタンスが 0 になる最も近い周波数を探します（その半分〜2 倍の範囲）`} multiline w={260}>
          <Button variant="default" leftSection={<IconTarget size={14} />} loading={resonance.running} onClick={findResonance}>
            共振点
          </Button>
        </Tooltip>
        {sweep.running && (
          <Button variant="default" leftSection={<IconPlayerStop size={14} />} onClick={stop} disabled={sweep.stopping}>
            {sweep.stopping ? '中止中…' : '中止'}
          </Button>
        )}
        {sweep.progress && (
          <Text size="sm" c="dimmed" mb={6} className="num" style={{ whiteSpace: 'nowrap' }}>
            {sweep.running ? '計算中' : '中止'} {sweep.progress.done} / {sweep.progress.total} 点
          </Text>
        )}
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
              { value: 'patterns', label: '指向性' },
              { value: 'table', label: '表' },
            ]}
            value={view}
            onChange={(v) => setView(v as View)}
          />
        )}
      </Group>
      {resonance.search && (
        <ResonanceRow
          search={resonance.search}
          stale={resonance.stale}
          onCenter={(mhz) => {
            setRange(sweepAround(mhz));
            run();
          }}
        />
      )}
      {data && view !== 'patterns' && <MatchRow data={data} stale={sweep.stale} />}
      {!data ? (
        <Text size="sm" c="dimmed" mt="md" ta="center">
          範囲を指定して「計算」を押すと、SWR・インピーダンス・利得・F/B 比と指向性の周波数特性を表示します（2° 分解能で計算）。
        </Text>
      ) : view === 'patterns' ? (
        <SweepPatterns data={data} opacity={opacity} />
      ) : view === 'chart' ? (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gridTemplateRows: '1fr 1fr', gap: 12, flex: 1, minHeight: 280, opacity }}>
          <LineChart title={matched ? 'SWR（整合回路あり）' : 'SWR'} unit="" x={x} hover={hover} onHover={setHover} yMin={1} yMax={10} markerX={designFreq}
            series={
              matched
                ? [
                    { label: '整合後', values: matched.map((m) => m?.swr ?? NaN), color: 'var(--series-1)' },
                    { label: '整合前', values: data.map((p) => p.swr), color: 'var(--series-2)', dash: '5 3' },
                  ]
                : [{ label: 'SWR', values: data.map((p) => p.swr), color: 'var(--series-1)' }]
            } />
          {matched && mode === 'swrZ' ? (
            <LineChart title="インピーダンス（整合後）" unit="Ω" x={x} hover={hover} onHover={setHover} markerX={designFreq}
              series={[
                { label: 'R', values: matched.map((m) => m?.impedance?.re ?? NaN), color: 'var(--series-1)' },
                { label: 'jX', values: matched.map((m) => m?.impedance?.im ?? NaN), color: 'var(--series-2)', dash: '5 3' },
              ]} />
          ) : (
            <LineChart title="インピーダンス" unit="Ω" x={x} hover={hover} onHover={setHover} markerX={designFreq}
              series={[
                { label: 'R', values: data.map((p) => p.impedance.re), color: 'var(--series-1)' },
                { label: 'jX', values: data.map((p) => p.impedance.im), color: 'var(--series-2)', dash: '5 3' },
              ]} />
          )}
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
              {matched && (
                <>
                  <Table.Th ta="right">整合後 R (Ω)</Table.Th>
                  <Table.Th ta="right">整合後 jX (Ω)</Table.Th>
                  <Table.Th ta="right">整合後 SWR</Table.Th>
                </>
              )}
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {data.map((p, i) => (
              <Table.Tr key={p.frequencyMhz}>
                <Table.Td ta="right">{p.frequencyMhz.toFixed(4)}</Table.Td>
                <Table.Td ta="right">{p.impedance.re.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.impedance.im.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{swrText(p.swr)}</Table.Td>
                <Table.Td ta="right">{p.gainDbi.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.fbDb.toFixed(2)}</Table.Td>
                <Table.Td ta="right">{p.elevationDeg.toFixed(1)}</Table.Td>
                {matched && (
                  <>
                    <Table.Td ta="right">{matched[i]?.impedance?.re.toFixed(2) ?? ''}</Table.Td>
                    <Table.Td ta="right">{matched[i]?.impedance?.im.toFixed(2) ?? ''}</Table.Td>
                    <Table.Td ta="right">{matched[i] ? swrText(matched[i]!.swr) : ''}</Table.Td>
                  </>
                )}
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      )}
    </Stack>
  );
}

/** The resonance found: frequency, impedance, and the series circuit there (MMANA's 推測計算). */
function ResonanceRow({ search, stale, onCenter }: { search: ResonanceSearch; stale: boolean; onCenter: (mhz: number) => void }) {
  const r = search.resonance;
  if (!r) {
    return (
      <Text size="sm" c="dimmed" style={{ opacity: stale ? 0.55 : 1 }}>
        {search.minMhz.toFixed(3)}〜{search.maxMhz.toFixed(3)} MHz に共振点はありません（{search.calculations} 回計算）
      </Text>
    );
  }
  const c = r.circuit;
  return (
    <Stack gap={4} style={{ opacity: stale ? 0.55 : 1 }}>
      <Group gap="md" wrap="wrap" className="num">
        <Text size="sm" fw={600}>
          共振 {r.frequencyMhz.toFixed(4)} MHz
        </Text>
        <Text size="sm">
          R {r.impedance.re.toFixed(2)} Ω・jX {r.impedance.im.toFixed(2)} Ω・SWR {r.swr >= 99999 ? '∞' : r.swr.toFixed(2)}
        </Text>
        {c && (
          <Tooltip label="共振点の近くを直列 RLC 回路とみなした値（MMANA の推測計算）。帯域幅は共振周波数 ÷ Q" multiline w={260}>
            <Text size="sm" c="dimmed">
              Q {c.q.toFixed(1)}・帯域幅 {c.bandwidthKhz.toFixed(0)} kHz・L {c.inductanceUh.toFixed(3)} µH・C {c.capacitancePf.toFixed(2)} pF
            </Text>
          </Tooltip>
        )}
        <Button size="compact-xs" variant="light" onClick={() => onCenter(r.frequencyMhz)}>
          ここを中心に周波数特性
        </Button>
        {stale && (
          <Badge color="yellow" variant="light" leftSection={<IconAlertTriangle size={12} />}>
            再計算が必要
          </Badge>
        )}
      </Group>
      {!r.converged && (
        <Alert color="yellow" variant="light" p={6} icon={<IconAlertTriangle size={14} />}>
          <Text size="xs">リアクタンスがここで 0 をまたいで跳んでいます。周波数で自動分割の分割数が変わるためで、共振周波数はこの付近です。</Text>
        </Alert>
      )}
    </Stack>
  );
}

type Matched = NonNullable<SweepMatchState['result']>['points'][number];

/** The matched point of each sweep point (by frequency: points still arriving have none yet); null with no matching. */
function matchedAt(data: SweepPoint[] | null, match: SweepMatchState | null): (Matched | undefined)[] | null {
  if (!data || !match?.result) return null;
  const by = new Map(match.result.points.map((p) => [p.frequencyMhz, p]));
  return data.map((p) => by.get(p.frequencyMhz));
}

const MATCH_MODES: { value: MatchMode; label: string }[] = [
  { value: 'off', label: 'なし' },
  { value: 'swr', label: 'SWR' },
  { value: 'swrZ', label: 'SWR と Z' },
];

const element = (e: Lumped) => (e.kind === 'L' ? `L ${sig(e.value, 4)} µH` : `C ${sig(e.value, 4)} pF`);

/** The SWR bandwidths under 1.5 and 2 (MMANA's ⇔ … KHz(SWR<1.5) on the SWR chart). */
function bandText(points: { frequencyMhz: number; swr: number }[]): string {
  const band = (limit: number) => {
    const b = swrBandwidth(points, limit);
    return `< ${limit}：${b ? `${b.open ? '≥ ' : ''}${b.khz.toFixed(1)} kHz` : 'なし'}`;
  };
  return `${band(1.5)}・${band(2)}`;
}

/** 整合回路 (the マッチング of the 周波数特性 dialog) and the SWR bandwidths with and without it. */
function MatchRow({ data, stale }: { data: SweepPoint[]; stale: boolean }) {
  const matching = useApp((s) => s.sweep.matching);
  const match = useApp((s) => s.sweep.match);
  const lineOhm = useApp((s) => s.sweep.basis?.project.environment.swrReference.r);
  const set = useApp((s) => s.setSweepMatching);
  const net = match?.result?.network;
  const matchedPoints = match?.result?.points.filter((p) => data.some((d) => d.frequencyMhz === p.frequencyMhz));
  return (
    <Stack gap={4} style={{ opacity: stale ? 0.55 : 1 }}>
      <Group gap="xs" wrap="wrap" align="center">
        <Tooltip
          label="整合周波数のインピーダンスから L 形の整合回路（2 素子）を求め、その L・C の値のまま周波数特性に入れます。素子の損失は含みません"
          multiline
          w={280}
        >
          <Text size="sm">整合回路</Text>
        </Tooltip>
        <SegmentedControl size="xs" value={matching.mode} data={MATCH_MODES} onChange={(v) => set({ mode: v as MatchMode })} aria-label="整合回路を入れるグラフ" />
        {matching.mode !== 'off' && (
          <>
            <Checkbox size="xs" label="中心周波数で整合" checked={matching.atCentre} onChange={(e) => set({ atCentre: e.currentTarget.checked })} />
            {!matching.atCentre && (
              <FieldNumber
                size="xs"
                w={120}
                aria-label="整合周波数"
                value={matching.frequencyMhz}
                decimalScale={4}
                rightSection={<Text size="xs" c="dimmed">MHz</Text>}
                rightSectionWidth={40}
                onCommit={(v) => set({ frequencyMhz: v })}
              />
            )}
            {match?.running && <Loader size="xs" />}
            {net && match && (
              <Text size="xs" className="num">
                {match.frequencyMhz.toFixed(4)} MHz で {sig(lineOhm, 4)} Ω に整合：
                {!net.shunt && !net.series
                  ? '整合済み（素子なし）'
                  : net.shuntSide === 'line'
                    ? [net.series && `アンテナに直列 ${element(net.series)}`, net.shunt && `給電線側に並列 ${element(net.shunt)}`].filter(Boolean).join('、')
                    : [net.shunt && `アンテナに並列 ${element(net.shunt)}`, net.series && `給電線へ直列 ${element(net.series)}`].filter(Boolean).join('、')}
              </Text>
            )}
          </>
        )}
      </Group>
      {matching.mode !== 'off' && match?.error && (
        <Text size="xs" c="var(--diag-error)">
          {match.error}
        </Text>
      )}
      <Group gap="md" wrap="wrap" className="num">
        <Text size="xs" c="dimmed">
          SWR 帯域幅{matchedPoints ? '（整合前）' : ''} {bandText(data)}
        </Text>
        {matchedPoints && (
          <Text size="xs" fw={600}>
            整合後 {bandText(matchedPoints)}
          </Text>
        )}
      </Group>
    </Stack>
  );
}
