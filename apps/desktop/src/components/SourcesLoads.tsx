import { ActionIcon, Button, Group, Select, Stack, Switch, Text, Title } from '@mantine/core';
import { IconPlus, IconTrash } from '@tabler/icons-react';
import type { Load, LoadKind } from '../api/types';
import { newLoad, newSource } from '../domain/project';
import { useApp } from '../state/store';
import { NumCell, TextCell } from './Cells';

const PULSE_HELP = 'w1c = ワイヤー1の中央、w2b / w2e = 始点 / 終点、w2b3 = 始点から3番目、数字のみ = パルス番号';

const validPulse = (t: string) => /^(w\d*[bce]?-?\d*|\d+)$/i.test(t.trim());

export function SourcesLoads() {
  return (
    <Stack gap="lg" p="sm">
      <Sources />
      <Loads />
    </Stack>
  );
}

function Sources() {
  const sources = useApp((s) => s.project.antenna.sources);
  const edit = useApp((s) => s.edit);
  return (
    <section>
      <Group justify="space-between" mb={4}>
        <Title order={6}>給電点</Title>
        <Button variant="light" leftSection={<IconPlus size={14} />} onClick={() => edit((p) => void p.antenna.sources.push(newSource()))}>
          給電点を追加
        </Button>
      </Group>
      <table className="grid-table" aria-label="給電点">
        <thead>
          <tr>
            <th style={{ textAlign: 'center' }}>No.</th>
            <th className="left" title={PULSE_HELP}>
              位置（パルス）
            </th>
            <th>位相 (°)</th>
            <th>電圧 (V)</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {sources.map((s, i) => (
            <tr key={i}>
              <td className="index num">{i + 1}</td>
              <td>
                <TextCell
                  ariaLabel={`給電点 ${i + 1} の位置`}
                  value={s.pulse}
                  validate={validPulse}
                  onCommit={(t) => edit((p) => void (p.antenna.sources[i].pulse = t.trim()))}
                />
              </td>
              <td>
                <NumCell ariaLabel={`給電点 ${i + 1} の位相`} value={s.phaseDeg} digits={2} onCommit={(v) => edit((p) => void (p.antenna.sources[i].phaseDeg = v))} />
              </td>
              <td>
                <NumCell ariaLabel={`給電点 ${i + 1} の電圧`} value={s.voltage} digits={3} onCommit={(v) => edit((p) => void (p.antenna.sources[i].voltage = v))} />
              </td>
              <td style={{ width: 32 }}>
                <ActionIcon size="sm" color="red" className="row-actions" aria-label={`給電点 ${i + 1} を削除`} onClick={() => edit((p) => void p.antenna.sources.splice(i, 1))}>
                  <IconTrash size={14} />
                </ActionIcon>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <Text size="xs" c="dimmed" mt={6}>
        {PULSE_HELP}
      </Text>
    </section>
  );
}

const LOAD_KINDS: { value: LoadKind['kind']; label: string }[] = [
  { value: 'lc', label: 'L・C・Q' },
  { value: 'impedance', label: 'R + jX' },
  { value: 'sDomain', label: 'S 領域' },
];

function changeKind(load: Load, kind: LoadKind['kind']): Load {
  const pulse = load.pulse;
  switch (kind) {
    case 'lc':
      return { pulse, kind, inductanceUh: 0, capacitancePf: 0, q: 0 };
    case 'impedance':
      return { pulse, kind, r: 0, x: 0 };
    case 'sDomain':
      return { pulse, kind, coefficients: [[1, 1]] };
  }
}

function Loads() {
  const loads = useApp((s) => s.project.antenna.loads);
  const enabled = useApp((s) => s.project.antenna.loadsEnabled);
  const edit = useApp((s) => s.edit);
  return (
    <section>
      <Group justify="space-between" mb={4}>
        <Group gap="sm">
          <Title order={6}>負荷（集中定数）</Title>
          <Switch label="計算に含める" checked={enabled} onChange={(e) => edit((p) => void (p.antenna.loadsEnabled = e.currentTarget.checked))} />
        </Group>
        <Button variant="light" leftSection={<IconPlus size={14} />} onClick={() => edit((p) => void p.antenna.loads.push(newLoad()))}>
          負荷を追加
        </Button>
      </Group>
      {loads.length === 0 ? (
        <Text size="xs" c="dimmed">
          負荷はありません。トラップやローディングコイルを入れる場合に追加します。
        </Text>
      ) : (
        <table className="grid-table" aria-label="負荷">
          <thead>
            <tr>
              <th style={{ textAlign: 'center' }}>No.</th>
              <th className="left">位置</th>
              <th className="left">種類</th>
              <th colSpan={3} className="left">
                値
              </th>
              <th />
            </tr>
          </thead>
          <tbody>
            {loads.map((l, i) => (
              <tr key={i}>
                <td className="index num">{i + 1}</td>
                <td style={{ width: 80 }}>
                  <TextCell ariaLabel={`負荷 ${i + 1} の位置`} value={l.pulse} validate={validPulse} onCommit={(t) => edit((p) => void (p.antenna.loads[i].pulse = t.trim()))} />
                </td>
                <td style={{ width: 110, padding: '0 4px' }}>
                  <Select
                    aria-label={`負荷 ${i + 1} の種類`}
                    data={LOAD_KINDS}
                    value={l.kind}
                    onChange={(k) => k && edit((p) => void (p.antenna.loads[i] = changeKind(p.antenna.loads[i], k as LoadKind['kind'])))}
                    comboboxProps={{ withinPortal: true }}
                    variant="unstyled"
                  />
                </td>
                <LoadValues load={l} index={i} />
                <td style={{ width: 32 }}>
                  <ActionIcon size="sm" color="red" className="row-actions" aria-label={`負荷 ${i + 1} を削除`} onClick={() => edit((p) => void p.antenna.loads.splice(i, 1))}>
                    <IconTrash size={14} />
                  </ActionIcon>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function Labeled({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <td>
      <Group gap={2} wrap="nowrap">
        <Text size="xs" c="dimmed" pl={6} style={{ whiteSpace: 'nowrap' }}>
          {label}
        </Text>
        {children}
      </Group>
    </td>
  );
}

function LoadValues({ load, index }: { load: Load; index: number }) {
  const edit = useApp((s) => s.edit);
  const set = (field: string) => (v: number) =>
    edit((p) => {
      (p.antenna.loads[index] as unknown as Record<string, number>)[field] = v;
    });
  switch (load.kind) {
    case 'lc':
      return (
        <>
          <Labeled label="L µH">
            <NumCell ariaLabel={`負荷 ${index + 1} の L`} value={load.inductanceUh} onCommit={set('inductanceUh')} />
          </Labeled>
          <Labeled label="C pF">
            <NumCell ariaLabel={`負荷 ${index + 1} の C`} value={load.capacitancePf} onCommit={set('capacitancePf')} />
          </Labeled>
          <Labeled label="Q">
            <NumCell ariaLabel={`負荷 ${index + 1} の Q`} value={load.q} digits={1} onCommit={set('q')} />
          </Labeled>
        </>
      );
    case 'impedance':
      return (
        <>
          <Labeled label="R Ω">
            <NumCell ariaLabel={`負荷 ${index + 1} の R`} value={load.r} digits={3} onCommit={set('r')} />
          </Labeled>
          <Labeled label="jX Ω">
            <NumCell ariaLabel={`負荷 ${index + 1} の X`} value={load.x} digits={3} onCommit={set('x')} />
          </Labeled>
          <td />
        </>
      );
    case 'sDomain':
      return (
        <td colSpan={3}>
          <Text size="xs" c="dimmed" px={6}>
            {load.coefficients.length} 次の S 領域関数（係数の編集は .maa ファイルで行ってください）
          </Text>
        </td>
      );
  }
}
