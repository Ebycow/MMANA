// Dialogs of the wire tools: アンテナのサイズ (拡大・縮小) and 検索と置換.
import { Button, Checkbox, Group, Modal, SegmentedControl, Select, Stack, Text, TextInput } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useState } from 'react';
import type { Point3 } from '../api/types';
import {
  lengthToDisplay,
  lengthUnit,
  lengthWavelength,
  radiusToDisplay,
  radiusUnit,
  radiusWavelength,
  strDbl,
  type Units,
} from '../domain/elements';
import { evaluate } from '../domain/expr';
import { wavelength } from '../domain/project';
import { formatSegmentation, parseSegmentation } from '../domain/segmentation';
import {
  ALL_SCALE_TARGETS,
  frequencyForScale,
  replaceInWires,
  scaleAntenna,
  scaleForFrequency,
  type CoordinateScope,
  type ReplaceTarget,
  type ScaleTargets,
} from '../domain/wireTools';
import { useDisplay } from '../state/display';
import { useApp } from '../state/store';
import { canEditWires } from '../state/wireCommands';
import { useWireTools, type WireColumn } from '../state/wireTools';

function refuseWhileBusy(): boolean {
  if (canEditWires()) return false;
  notifications.show({ color: 'yellow', message: 'エレメント編集・最適化の画面を閉じてから実行してください' });
  return true;
}

/** The units the wire table shows lengths in. */
function useWireUnits(): Units {
  const freq = useApp((s) => s.project.antenna.frequencyMhz);
  const display = useDisplay();
  const lambda = wavelength(freq);
  return { wavelengths: display.wavelengths && lambda > 0, millimetres: display.millimetres, lambda };
}

export function WireTools() {
  const scaleOpen = useWireTools((s) => s.scaleOpen);
  const replaceColumn = useWireTools((s) => s.replaceColumn);
  return (
    <>
      {scaleOpen && <ScaleDialog />}
      {replaceColumn && <ReplaceDialog column={replaceColumn} />}
    </>
  );
}

const TARGET_LABELS: [keyof ScaleTargets, string][] = [
  ['x', 'X 軸'],
  ['y', 'Y 軸'],
  ['z', 'Z 軸'],
  ['radius', 'ワイヤー半径 (R)'],
];

/**
 * `TWireScaleDlg`: the 目標周波数 and the 換算倍率 follow each other; the
 * value typed last decides, and the other is shown to six decimals.
 */
function ScaleDialog() {
  const close = () => useWireTools.getState().setScaleOpen(false);
  const design = useApp((s) => s.project.antenna.frequencyMhz);
  const [freq, setFreq] = useState(design);
  const [scale, setScale] = useState(1);
  const [freqText, setFreqText] = useState(strDbl(design));
  const [scaleText, setScaleText] = useState(strDbl(1));
  const [targets, setTargets] = useState<ScaleTargets>(ALL_SCALE_TARGETS);
  const ctx = { wavelength: wavelength(design) };
  const freqValue = evaluate(freqText, ctx);
  const scaleValue = evaluate(scaleText, ctx);
  const coords = targets.x || targets.y || targets.z;
  const changes = scale !== 1 && (coords || targets.radius);

  const typeFreq = (t: string) => {
    setFreqText(t);
    const d = evaluate(t, ctx);
    if (d == null || !(d > 0) || d === freq) return;
    const s = scaleForFrequency(design, d);
    setFreq(d);
    setScale(s);
    setScaleText(strDbl(s));
  };
  const typeScale = (t: string) => {
    setScaleText(t);
    const d = evaluate(t, ctx);
    if (d == null || !(d > 0) || d === scale) return;
    const f = frequencyForScale(design, d);
    setScale(d);
    setFreq(f);
    setFreqText(strDbl(f));
  };

  const apply = () => {
    if (refuseWhileBusy()) return;
    const app = useApp.getState();
    const scaled = scaleAntenna(app.project.antenna, scale, freq, targets);
    if (scaled) {
      // like the original, which sets the calculation frequency (cfq) with the design frequency
      const followDesign = coords && app.options.frequencyMhz != null;
      app.edit((p) => void (p.antenna = scaled), {
        label: `アンテナのサイズを ${strDbl(scale)} 倍`,
        ...(followDesign && { options: { frequencyMhz: null } }),
      });
    }
    close();
  };

  return (
    <Modal opened onClose={close} title="アンテナのサイズ（拡大・縮小）" size="sm">
      <Stack gap="sm">
        <Text size="sm">
          設計周波数 <b>{strDbl(design)} MHz</b>
        </Text>
        <TextInput
          label="目標周波数 (MHz)"
          value={freqText}
          onChange={(e) => typeFreq(e.currentTarget.value)}
          error={freqText && !(freqValue != null && freqValue > 0) ? '正の値を入力してください' : null}
          data-autofocus
        />
        <TextInput
          label="換算倍率"
          value={scaleText}
          onChange={(e) => typeScale(e.currentTarget.value)}
          error={scaleText && !(scaleValue != null && scaleValue > 0) ? '正の値を入力してください' : null}
        />
        <Stack gap={6}>
          <Text size="sm" fw={500}>
            対象
          </Text>
          <Group gap="md">
            {TARGET_LABELS.map(([key, label]) => (
              <Checkbox key={key} label={label} checked={targets[key]} onChange={(e) => setTargets({ ...targets, [key]: e.currentTarget.checked })} />
            ))}
          </Group>
        </Stack>
        <Text size="xs" c="dimmed">
          原点を中心に、ワイヤーの座標と半径に換算倍率を掛けます。座標を変えると設計周波数と計算周波数は目標周波数になります。
          負荷・スタックの間隔・地上高・組み合わせワイヤの定義は変わりません。
        </Text>
        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={close}>
            キャンセル
          </Button>
          <Button disabled={!changes} onClick={apply}>
            変更
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

type ReplaceKind = ReplaceTarget['kind'];

const COORD_COLUMNS: Partial<Record<WireColumn, { end: 'start' | 'end'; axis: keyof Point3 }>> = {
  X1: { end: 'start', axis: 'x' },
  Y1: { end: 'start', axis: 'y' },
  Z1: { end: 'start', axis: 'z' },
  X2: { end: 'end', axis: 'x' },
  Y2: { end: 'end', axis: 'y' },
  Z2: { end: 'end', axis: 'z' },
};

const SCOPES: { value: CoordinateScope; label: string }[] = [
  { value: 'all', label: 'すべて' },
  { value: 'x', label: 'X 座標のみ' },
  { value: 'y', label: 'Y 座標のみ' },
  { value: 'z', label: 'Z 座標のみ' },
];

/**
 * `TValRepDlg`, opened on a wire-table column: a coordinate column searches
 * coordinates (all, or one axis), R the radii and SEG the segmentations.
 * Both values start as the current wire's value in that column.
 */
function ReplaceDialog({ column }: { column: WireColumn }) {
  const close = () => useWireTools.getState().closeReplace();
  const units = useWireUnits();
  const initialKind: ReplaceKind = column === 'R' ? 'radius' : column === 'SEG' ? 'segmentation' : 'coordinate';
  const initialText = (kind: ReplaceKind): string => {
    const { project, selectedWire } = useApp.getState();
    const w = selectedWire == null ? undefined : project.antenna.wires[selectedWire];
    if (!w) return '';
    if (kind === 'radius') return strDbl(radiusToDisplay(w.radius, units));
    if (kind === 'segmentation') return formatSegmentation(w.segmentation);
    const c = COORD_COLUMNS[column] ?? COORD_COLUMNS.X1!;
    return strDbl(lengthToDisplay(w[c.end][c.axis], units));
  };
  const [kind, setKind] = useState<ReplaceKind>(initialKind);
  const [scope, setScope] = useState<CoordinateScope>('all');
  const [absolute, setAbsolute] = useState(true);
  const [search, setSearch] = useState(() => initialText(initialKind));
  const [replacement, setReplacement] = useState(() => initialText(initialKind));

  const number = (t: string) => evaluate(t, { wavelength: kind === 'radius' ? radiusWavelength(units) : lengthWavelength(units) });
  const target = ((): ReplaceTarget | null => {
    if (kind === 'segmentation') {
      const s = parseSegmentation(search);
      const r = parseSegmentation(replacement);
      return s && r && { kind, search: s, replacement: r };
    }
    const s = number(search);
    const r = number(replacement);
    if (s == null || r == null) return null;
    return kind === 'radius' ? { kind, search: s, replacement: r } : { kind, scope, absolute, search: s, replacement: r };
  })();
  const unreadable = (t: string) => t !== '' && (kind === 'segmentation' ? parseSegmentation(t) == null : number(t) == null);
  const unit = kind === 'coordinate' ? lengthUnit(units) : kind === 'radius' ? radiusUnit(units) : null;

  const pickKind = (k: ReplaceKind) => {
    setKind(k);
    setSearch(initialText(k));
    setReplacement(initialText(k));
  };

  const apply = () => {
    if (!target || refuseWhileBusy()) return;
    const app = useApp.getState();
    const r = replaceInWires(app.project.antenna.wires, target, units);
    if (r.count === 0) {
      notifications.show({ message: '一致する値はありません' });
      return;
    }
    app.edit((p) => void (p.antenna.wires = r.wires), { label: `検索と置換（${r.count} か所）` });
    notifications.show({ message: `${r.count} か所を置換しました` });
    close();
  };

  return (
    <Modal opened onClose={close} title="検索と置換" size="sm">
      <Stack gap="sm">
        <SegmentedControl
          aria-label="検索する列"
          value={kind}
          onChange={(v) => pickKind(v as ReplaceKind)}
          data={[
            { value: 'coordinate', label: '座標' },
            { value: 'radius', label: '半径 R' },
            { value: 'segmentation', label: 'SEG' },
          ]}
        />
        <TextInput
          label={unit ? `検索する値 (${unit})` : '検索する値'}
          value={search}
          onChange={(e) => setSearch(e.currentTarget.value)}
          error={unreadable(search) ? '値を読めません' : null}
          data-autofocus
        />
        <TextInput
          label={unit ? `置き換える値 (${unit})` : '置き換える値'}
          value={replacement}
          onChange={(e) => setReplacement(e.currentTarget.value)}
          error={unreadable(replacement) ? '値を読めません' : null}
        />
        {kind === 'coordinate' && (
          <Group gap="md" align="end">
            <Select
              label="検索する座標"
              w={150}
              allowDeselect={false}
              value={scope}
              onChange={(v) => v && setScope(v as CoordinateScope)}
              data={SCOPES}
            />
            <Checkbox mb={8} label="絶対値を検索" checked={absolute} onChange={(e) => setAbsolute(e.currentTarget.checked)} />
          </Group>
        )}
        <Text size="xs" c="dimmed">
          すべてのワイヤーで、表に表示される値（小数点以下 6 桁）が一致するものを置き換えます。
          {kind === 'coordinate' && '「絶対値を検索」では符号違いの値も、符号を保って置き換えます。'}
        </Text>
        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={close}>
            キャンセル
          </Button>
          <Button disabled={!target} onClick={apply}>
            すべて置換
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
