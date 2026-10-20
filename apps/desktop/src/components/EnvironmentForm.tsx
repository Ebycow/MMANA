import { ActionIcon, Button, Group, SegmentedControl, Select, SimpleGrid, Stack, Switch, Text, Title } from '@mantine/core';
import { IconPlus, IconTrash } from '@tabler/icons-react';
import type { GroundKind, WireMaterial } from '../api/types';
import { useApp } from '../state/store';
import { NumCell } from './Cells';
import { FieldNumber } from './Fields';

const GROUNDS: { value: GroundKind; label: string }[] = [
  { value: 'freeSpace', label: '自由空間' },
  { value: 'perfect', label: '完全大地' },
  { value: 'real', label: 'リアルグランド' },
];

const MATERIALS: { value: WireMaterial; label: string }[] = [
  { value: 'lossless', label: '無損失' },
  { value: 'copper', label: '銅線' },
  { value: 'copperPipe', label: '銅パイプ' },
  { value: 'aluminum', label: 'アルミ線' },
  { value: 'aluminumPipe', label: 'アルミパイプ' },
  { value: 'iron', label: '鉄線' },
  { value: 'ironPipe', label: '鉄パイプ' },
];

export function EnvironmentForm() {
  const env = useApp((s) => s.project.environment);
  const edit = useApp((s) => s.edit);
  const hasGround = env.ground !== 'freeSpace';

  return (
    <Stack gap="lg" p="sm" maw={640}>
      <section>
        <Title order={6} mb="xs">
          大地
        </Title>
        <SegmentedControl
          fullWidth
          data={GROUNDS}
          value={env.ground}
          onChange={(v) => edit((p) => void (p.environment.ground = v as GroundKind))}
        />
        <Text size="xs" c="dimmed" mt={6}>
          {env.ground === 'freeSpace' && '大地の影響を含めずに計算します。'}
          {env.ground === 'perfect' && '完全導体の大地として計算します。'}
          {env.ground === 'real' &&
            '電流分布は完全大地で、放射パターンは下記の大地定数による反射を含めて計算します（MININEC の方式）。'}
        </Text>
        <SimpleGrid cols={2} mt="sm">
          <FieldNumber
            label="地上高 (m)"
            description="すべてのワイヤーの Z に加算されます"
            disabled={!hasGround}
            value={env.height}
            decimalScale={3}
            onCommit={(v) => edit((p) => void (p.environment.height = v))}
          />
          <Select
            label="線材"
            description="導体損失を負荷として加えます"
            data={MATERIALS}
            value={env.wireMaterial}
            onChange={(v) => v && edit((p) => void (p.environment.wireMaterial = v as WireMaterial))}
          />
        </SimpleGrid>
      </section>

      {env.ground === 'real' && <RealGround />}

      <section>
        <Title order={6} mb="xs">
          評価の基準
        </Title>
        <SimpleGrid cols={2}>
          <FieldNumber label="SWR 基準インピーダンス R (Ω)" value={env.swrReference.r} min={0.001} onCommit={(v) => edit((p) => void (p.environment.swrReference.r = v))} />
          <FieldNumber label="SWR 基準インピーダンス jX (Ω)" value={env.swrReference.x} onCommit={(v) => edit((p) => void (p.environment.swrReference.x = v))} />
          <FieldNumber
            label="F/B 比のバック範囲 水平 (°)"
            value={env.fbRange.horizontal}
            min={0}
            max={360}
            allowDecimal={false}
            onCommit={(v) => edit((p) => void (p.environment.fbRange.horizontal = v))}
          />
          <FieldNumber
            label="F/B 比のバック範囲 垂直 (°)"
            value={env.fbRange.vertical}
            min={0}
            max={90}
            allowDecimal={false}
            onCommit={(v) => edit((p) => void (p.environment.fbRange.vertical = v))}
          />
        </SimpleGrid>
      </section>
    </Stack>
  );
}

function RealGround() {
  const rg = useApp((s) => s.project.environment.realGround);
  const edit = useApp((s) => s.edit);
  const media = rg?.media ?? [];
  return (
    <section>
      <Group justify="space-between" mb={4}>
        <Title order={6}>大地定数</Title>
        <Button
          variant="light"
          leftSection={<IconPlus size={14} />}
          disabled={media.length >= 15}
          onClick={() =>
            edit((p) => void p.environment.realGround!.media.push({ permittivity: 13, conductivity: 0.005, boundary: null, height: null }))
          }
        >
          媒質を追加
        </Button>
      </Group>
      <table className="grid-table" aria-label="大地の媒質">
        <thead>
          <tr>
            <th style={{ textAlign: 'center' }}>No.</th>
            <th>比誘電率</th>
            <th>導電率 (S/m)</th>
            <th>次の媒質までの距離 (m)</th>
            <th>高さ (m)</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {media.map((m, i) => {
            const setM = (field: 'permittivity' | 'conductivity' | 'boundary' | 'height') => (v: number) =>
              edit((p) => void (p.environment.realGround!.media[i][field] = v));
            return (
              <tr key={i}>
                <td className="index num">{i + 1}</td>
                <td>
                  <NumCell ariaLabel={`媒質 ${i + 1} の比誘電率`} value={m.permittivity} digits={2} onCommit={setM('permittivity')} />
                </td>
                <td>
                  <NumCell ariaLabel={`媒質 ${i + 1} の導電率`} value={m.conductivity} digits={5} onCommit={setM('conductivity')} />
                </td>
                <td>
                  {i === media.length - 1 ? (
                    <Text size="xs" c="dimmed" ta="right" px={8}>
                      ∞
                    </Text>
                  ) : (
                    <NumCell ariaLabel={`媒質 ${i + 1} の境界`} value={m.boundary ?? 0} digits={2} onCommit={setM('boundary')} />
                  )}
                </td>
                <td>
                  <NumCell ariaLabel={`媒質 ${i + 1} の高さ`} value={m.height ?? 0} digits={2} onCommit={setM('height')} />
                </td>
                <td style={{ width: 32 }}>
                  <ActionIcon
                    size="sm"
                    color="red"
                    className="row-actions"
                    disabled={media.length <= 1}
                    aria-label={`媒質 ${i + 1} を削除`}
                    onClick={() => edit((p) => void p.environment.realGround!.media.splice(i, 1))}
                  >
                    <IconTrash size={14} />
                  </ActionIcon>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <Switch
        mt="sm"
        label="ラジアルによるグランドスクリーン（媒質の境界は同心円になります）"
        checked={rg?.screen != null}
        onChange={(e) =>
          edit((p) => void (p.environment.realGround!.screen = e.currentTarget.checked ? { radials: 100, radialRadius: 5 } : null))
        }
      />
      {rg?.screen && (
        <SimpleGrid cols={2} mt="xs">
          <FieldNumber
            label="ラジアルの本数"
            value={rg.screen.radials}
            min={1}
            allowDecimal={false}
            onCommit={(v) => edit((p) => void (p.environment.realGround!.screen!.radials = v))}
          />
          <FieldNumber
            label="ラジアルの半径"
            value={rg.screen.radialRadius}
            onCommit={(v) => edit((p) => void (p.environment.realGround!.screen!.radialRadius = v))}
          />
        </SimpleGrid>
      )}
    </section>
  );
}
