// エレメント編集 (MMANA's GrpWire dialog): wires joined at their ends are
// edited as elements — form, interval or position, width, height, perimeter,
// radius, segmentation. Like the original it edits a copy and writes the wires
// back on 反映 as one undoable change. Every edit is replayed by mmana-core from
// the antenna as it was when the dialog opened, so the table always shows what
// the engine will write.
import {
  ActionIcon,
  Alert,
  Button,
  Checkbox,
  Group,
  Menu,
  Modal,
  ScrollArea,
  SegmentedControl,
  Select,
  Slider,
  Stack,
  Text,
  TextInput,
  Tooltip,
  UnstyledButton,
} from '@mantine/core';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import {
  IconAlertTriangle,
  IconArrowBackUp,
  IconArrowForwardUp,
  IconChevronDown,
  IconDecimal,
  IconFileSpreadsheet,
  IconReplace,
  IconRowInsertTop,
  IconTrash,
} from '@tabler/icons-react';
import { Fragment, useEffect, useState } from 'react';
import type { ElementEdit, ElementKind, ElementOp, Segmentation } from '../api/types';
import { EngineError, files, transport } from '../api/transport';
import {
  COLUMN_LABELS,
  KIND_LABELS,
  KIND_MENU,
  acceptsValue,
  cellOp,
  cellValue,
  elementsCsv,
  lengthDigits,
  lengthUnit,
  lengthWavelength,
  radiusDigits,
  radiusUnit,
  radiusWavelength,
  replaceOps,
  strDbl,
  type ElementColumn,
  type Projection,
  type Units,
} from '../domain/elements';
import { evaluate } from '../domain/expr';
import { wavelength } from '../domain/project';
import { formatSegmentation, parseSegmentation } from '../domain/segmentation';
import { useDisplay } from '../state/display';
import { useCommandKeys } from '../state/input';
import { useApp } from '../state/store';
import { NumCell, TextCell, format } from './Cells';
import { ElementPreview } from './ElementPreview';

interface Settings {
  /** 間隔表記 (true) or 位置表記. */
  intervals: boolean;
  /** 全ての座標を同一比率で変更 (true) or 端点のみ変更. */
  scaleAll: boolean;
  view: Projection;
}

const DEFAULT_SETTINGS: Settings = { intervals: true, scaleAll: false, view: { azimuth: -50, elevation: 25 } };
const SETTINGS_KEY = 'mmana.element-editor';

// the dialog's check boxes are remembered per viewer, like MMANA's ini file
function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    return raw ? { ...DEFAULT_SETTINGS, ...(JSON.parse(raw) as Partial<Settings>) } : DEFAULT_SETTINGS;
  } catch {
    return DEFAULT_SETTINGS;
  }
}

function saveSettings(s: Settings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
  } catch {
    // not remembered; the defaults apply next time
  }
}

const VIEWS: { label: string; view: Projection }[] = [
  { label: '斜め', view: { azimuth: -50, elevation: 25 } },
  { label: '上', view: { azimuth: 0, elevation: 90 } },
  { label: '正面', view: { azimuth: 0, elevation: 0 } },
  { label: '横', view: { azimuth: -90, elevation: 0 } },
];

export function ElementEditor() {
  const open = useApp((s) => s.elementEditorOpen);
  return open ? <ElementEditorDialog onDone={() => useApp.getState().setElementEditorOpen(false)} /> : null;
}

function ElementEditorDialog({ onDone }: { onDone: () => void }) {
  // the antenna as it was when the dialog opened; every edit is replayed on it
  const [base] = useState(() => useApp.getState().project.antenna);
  const fileName = useApp((s) => s.fileName);
  const lambda = wavelength(base.frequencyMhz);

  const [steps, setSteps] = useState<ElementOp[][]>([]);
  const [undone, setUndone] = useState<ElementOp[][]>([]);
  const [result, setResult] = useState<{ steps: ElementOp[][]; edit: ElementEdit } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [focusCol, setFocusCol] = useState<ElementColumn>('width');
  // remounts the cells after a refused edit so they show the real values again
  const [revision, setRevision] = useState(0);
  const [replaceOpen, setReplaceOpen] = useState(false);
  const [settings, setSettingsState] = useState(loadSettings);
  // 波長表記 is shared with the wire table, and so is the main window's mm setting
  const wavelengths = useDisplay((s) => s.wavelengths);
  const millimetres = useDisplay((s) => s.millimetres);
  const setWavelengths = useDisplay((s) => s.setWavelengths);
  const setSettings = (patch: Partial<Settings>) => {
    const next = { ...settings, ...patch };
    setSettingsState(next);
    saveSettings(next);
  };

  useEffect(() => {
    let live = true;
    transport.editElements({ antenna: base, operations: steps.flat() }).then(
      (edit) => {
        if (!live) return;
        setError(null);
        if (!edit.applied.includes(false)) {
          setResult({ steps, edit });
          return;
        }
        // a refused edit changed nothing: drop it, so undo does not stop at it
        notifications.show({
          color: 'yellow',
          message:
            'この編集はできません。値が範囲外か、編集するとエレメントの形が崩れます（ワイヤが重なる、長さが 0 になる、表の寸法とワイヤが合わなくなる）。',
        });
        let k = 0;
        const kept = steps.map((s) => s.filter(() => edit.applied[k++])).filter((s) => s.length > 0);
        setSteps(kept);
        setResult({ steps: kept, edit });
        setRevision((r) => r + 1);
      },
      (e: unknown) => live && setError(e instanceof EngineError ? e.detail || e.title : String(e)),
    );
    return () => {
      live = false;
    };
  }, [base, steps]);

  const edit = result?.edit;
  const current = result != null && result.steps === steps;
  const rows = edit?.elements ?? [];
  const baseRow = edit?.base ?? 0;
  const units: Units = { wavelengths: wavelengths && lambda > 0, millimetres, lambda };
  const lu = lengthUnit(units);
  const lengthCols: ElementColumn[] = [settings.intervals ? 'interval' : 'position', 'width', 'height', 'perimeter'];

  const push = (ops: ElementOp[]) => {
    if (ops.length === 0) return;
    setSteps([...steps, ops]);
    setUndone([]);
  };
  const undo = () => {
    if (steps.length === 0) return;
    setUndone([steps[steps.length - 1], ...undone]);
    setSteps(steps.slice(0, -1));
  };
  const redo = () => {
    if (undone.length === 0) return;
    setSteps([...steps, undone[0]]);
    setUndone(undone.slice(1));
  };
  useCommandKeys({ undo, redo });

  const apply = () => {
    if (!edit || !current) return;
    if (steps.length === 0) return onDone();
    const app = useApp.getState();
    if (app.project.antenna !== base) {
      notifications.show({ color: 'red', title: 'エレメント編集を反映できません', message: '編集中にアンテナが変更されました。' });
      return onDone();
    }
    app.edit((p) => void (p.antenna = structuredClone(edit.antenna)));
    app.selectWire(null);
    if (edit.warnings.length) {
      notifications.show({ color: 'yellow', title: '給電点・負荷を更新しました', message: edit.warnings.join('\n') });
    }
    onDone();
  };

  const cancel = () => {
    if (steps.length === 0) return onDone();
    modals.openConfirmModal({
      title: 'エレメント編集の変更を破棄しますか？',
      children: <Text size="sm">ワイヤ定義の変更はアンテナに反映されません。</Text>,
      labels: { confirm: '破棄する', cancel: '編集を続ける' },
      confirmProps: { color: 'red' },
      onConfirm: onDone,
    });
  };

  const roundValues = (decimals: number) =>
    modals.openConfirmModal({
      title: '値を丸める',
      children: (
        <Text size="sm">
          すべてのエレメントの寸法・位置・半径を、小数点以下 {decimals} 桁（{lu}）に丸めます。
        </Text>
      ),
      labels: { confirm: '丸める', cancel: 'キャンセル' },
      onConfirm: () =>
        push([
          { op: 'roundValues', decimals, inWavelengths: units.wavelengths, inMillimetres: units.millimetres, scaleAll: settings.scaleAll },
        ]),
    });

  const exportCsv = async () => {
    try {
      const name = `${fileName.replace(/\.[^.]*$/, '') || '無題'}.csv`;
      await files.saveText(elementsCsv(rows, baseRow, units), name, { name: 'CSV ファイル', extension: 'csv' });
    } catch (e) {
      notifications.show({ color: 'red', title: 'CSV ファイルを保存できません', message: e instanceof EngineError ? e.detail : String(e) });
    }
  };

  // expressions: R is the wavelength in the unit the cell shows
  const exprWavelength = (col: ElementColumn) => (col === 'radius' ? radiusWavelength(units) : lengthWavelength(units));
  const digits = (col: ElementColumn) => (col === 'radius' ? radiusDigits(units) : lengthDigits(units));

  const numCell = (row: number, col: ElementColumn, isNew: boolean) => {
    const label = `エレメント ${row + 1} ${COLUMN_LABELS[col]}`;
    const cell = (value: number) => (
      <td key={col} style={{ minWidth: 56 }} onFocus={() => setFocusCol(col)}>
        <NumCell
          ariaLabel={label}
          value={value}
          digits={digits(col)}
          wavelength={exprWavelength(col)}
          validate={(v) => acceptsValue(col, v)}
          onCommit={(v) => push([cellOp(row, col, v, units, settings.scaleAll)])}
        />
      </td>
    );
    if (isNew) return cell(NaN);
    const v = cellValue(rows, baseRow, row, col, units);
    if (v == null) {
      return (
        <td key={col} className="readonly">
          {col === 'interval' ? '基準位置' : '複合'}
        </td>
      );
    }
    // positions count from the first element
    if (col === 'position' && row === 0) {
      return (
        <td key={col} className="readonly num">
          {format(v, digits(col))}
        </td>
      );
    }
    return cell(v);
  };

  const segCell = (row: number, s: Segmentation | null | undefined, isNew: boolean) => {
    if (!isNew && s == null) return <td className="readonly">複合</td>;
    return (
      <td style={{ minWidth: 50 }} onFocus={() => setFocusCol('segmentation')}>
        <TextCell
          ariaLabel={`エレメント ${row + 1} 分割`}
          align="right"
          value={s ? formatSegmentation(s) : ''}
          validate={(t) => parseSegmentation(t) != null}
          onCommit={(t) => push([{ op: 'setSegmentation', row, segmentation: parseSegmentation(t)! }])}
        />
      </td>
    );
  };

  const perimeterLabel = selected != null && rows[selected]?.xWidth ? 'Ｘ幅' : '全周';
  const duplicate = current ? edit?.duplicate : null;

  return (
    <Modal
      opened
      onClose={cancel}
      closeOnEscape={false}
      size="min(1280px, 96vw)"
      title={
        <Text fw={600}>
          エレメント編集
          <Text span c="dimmed" ml="xs" size="sm">
            {base.name}
          </Text>
        </Text>
      }
    >
      <Group gap="md" mb="xs" wrap="wrap">
        <SegmentedControl
          size="xs"
          value={settings.intervals ? 'interval' : 'position'}
          onChange={(v) => setSettings({ intervals: v === 'interval' })}
          data={[
            { value: 'interval', label: '間隔表記' },
            { value: 'position', label: '位置表記' },
          ]}
        />
        <Tooltip
          label="横幅・縦幅・全周を変えるとき、外側の端点だけを動かすか、すべての座標を同じ比率で拡大・縮小するか"
          multiline
          w={280}
        >
          <SegmentedControl
            size="xs"
            value={settings.scaleAll ? 'all' : 'edge'}
            onChange={(v) => setSettings({ scaleAll: v === 'all' })}
            data={[
              { value: 'edge', label: '端点のみ変更' },
              { value: 'all', label: '全ての座標を同一比率で変更' },
            ]}
          />
        </Tooltip>
        <Checkbox
          size="xs"
          label="波長表記"
          checked={units.wavelengths}
          disabled={!(lambda > 0)}
          onChange={(e) => setWavelengths(e.currentTarget.checked)}
        />
        <Menu position="bottom-start" shadow="md" width={230}>
          <Menu.Target>
            <Button size="xs" variant="default" rightSection={<IconChevronDown size={12} />}>
              操作
            </Button>
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Label>値を丸める</Menu.Label>
            {[2, 3, 4].map((d) => (
              <Menu.Item key={d} leftSection={<IconDecimal size={14} />} disabled={rows.length === 0} onClick={() => roundValues(d)}>
                小数点以下 {d} 桁
              </Menu.Item>
            ))}
            <Menu.Divider />
            <Menu.Item leftSection={<IconReplace size={14} />} disabled={rows.length === 0} onClick={() => setReplaceOpen(true)}>
              検索と置換…
            </Menu.Item>
            <Menu.Item leftSection={<IconFileSpreadsheet size={14} />} disabled={rows.length === 0} onClick={exportCsv}>
              CSV ファイルの作成…
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
        <Group gap={2} ml="auto">
          <Tooltip label="エレメント編集の操作を元に戻す">
            <ActionIcon aria-label="操作を元に戻す" variant="subtle" disabled={steps.length === 0} onClick={undo}>
              <IconArrowBackUp size={16} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label="やり直す">
            <ActionIcon aria-label="操作をやり直す" variant="subtle" disabled={undone.length === 0} onClick={redo}>
              <IconArrowForwardUp size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      <div style={{ display: 'flex', gap: 12, height: '60vh', minHeight: 340 }}>
        <ScrollArea style={{ flex: 1.7, minWidth: 0, border: '1px solid var(--panel-border)', borderRadius: 8 }} type="auto">
          <table className="grid-table" aria-label="エレメント">
            <thead>
              <tr>
                <th style={{ textAlign: 'center' }}>No.</th>
                <th className="left">形式</th>
                <th>
                  {settings.intervals ? '間隔' : '位置'} ({lu})
                </th>
                <th>横幅 ({lu})</th>
                <th>縦幅 ({lu})</th>
                <th>
                  <Tooltip label="ループは全周、それ以外は X 方向の長さ（Ｘ幅）" multiline w={220}>
                    <span>
                      {perimeterLabel} ({lu})
                    </span>
                  </Tooltip>
                </th>
                <th>R ({radiusUnit(units)})</th>
                <th>Seg.</th>
                <th>ワイヤ数</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {rows.map((r, i) => (
                <tr key={`${i}-${revision}`} className={selected === i ? 'selected' : undefined} onFocus={() => setSelected(i)}>
                  <td className="index num" onClick={() => setSelected(selected === i ? null : i)}>
                    {i + 1}
                  </td>
                  <td className="left" style={{ minWidth: 92, whiteSpace: 'nowrap' }}>
                    <KindMenu label={KIND_LABELS[r.kind]} row={i} onPick={(kind) => push([{ op: 'setKind', row: i, kind }])} />
                    {i === baseRow && (
                      <Text span size="xs" c="red" ml={4} title="間隔の基準になるエレメント（X = 0 に最も近い）">
                        基準
                      </Text>
                    )}
                  </td>
                  {lengthCols.map((col) => numCell(i, col, false))}
                  {numCell(i, 'radius', false)}
                  {segCell(i, r.segmentation, false)}
                  <td className="readonly num">{r.wireCount}</td>
                  <td style={{ width: 52 }}>
                    <Group gap={0} wrap="nowrap" className="row-actions">
                      <Tooltip label="この行に挿入（このエレメントを複製）">
                        <ActionIcon size="sm" aria-label={`エレメント ${i + 1} の前に挿入`} onClick={() => push([{ op: 'insert', row: i }])}>
                          <IconRowInsertTop size={14} />
                        </ActionIcon>
                      </Tooltip>
                      <Tooltip label={i === baseRow ? '基準エレメントは削除できません' : 'この行を削除'}>
                        <ActionIcon
                          size="sm"
                          color="red"
                          aria-label={`エレメント ${i + 1} を削除`}
                          disabled={i === baseRow}
                          onClick={() => {
                            push([{ op: 'delete', row: i }]);
                            setSelected(null);
                          }}
                        >
                          <IconTrash size={14} />
                        </ActionIcon>
                      </Tooltip>
                    </Group>
                  </td>
                </tr>
              ))}
              {edit && (
                <tr key={`new-${rows.length}-${revision}`}>
                  <td className="index">新規</td>
                  <td className="left">
                    <KindMenu label="形式を選ぶ" muted row={rows.length} onPick={(kind) => push([{ op: 'setKind', row: rows.length, kind }])} />
                  </td>
                  {lengthCols.map((col) => numCell(rows.length, col, true))}
                  {numCell(rows.length, 'radius', true)}
                  {segCell(rows.length, null, true)}
                  <td />
                  <td />
                </tr>
              )}
            </tbody>
          </table>
        </ScrollArea>

        <Stack gap={0} style={{ flex: 1, minWidth: 260, border: '1px solid var(--panel-border)', borderRadius: 8, overflow: 'hidden' }}>
          <div style={{ position: 'relative', flex: 1, background: 'var(--view3d-bg)' }}>
            {edit && (
              <ElementPreview
                wires={edit.antenna.wires}
                wireElements={edit.wireElements}
                sources={edit.antenna.sources}
                base={baseRow}
                selected={selected}
                view={settings.view}
                onSelect={setSelected}
              />
            )}
          </div>
          <Stack gap={6} p="xs" style={{ borderTop: '1px solid var(--panel-border)' }}>
            <Group gap="xs" wrap="nowrap">
              <Text size="xs" c="dimmed" w={28}>
                回転
              </Text>
              <Slider
                style={{ flex: 1 }}
                size="sm"
                min={-180}
                max={180}
                label={(v) => `${v}°`}
                value={settings.view.azimuth}
                onChange={(azimuth) => setSettings({ view: { ...settings.view, azimuth } })}
              />
            </Group>
            <Group gap="xs" wrap="nowrap">
              <Text size="xs" c="dimmed" w={28}>
                視点
              </Text>
              <Slider
                style={{ flex: 1 }}
                size="sm"
                min={-90}
                max={90}
                label={(v) => `${v}°`}
                value={settings.view.elevation}
                onChange={(elevation) => setSettings({ view: { ...settings.view, elevation } })}
              />
            </Group>
            <Group gap={4}>
              {VIEWS.map((v) => (
                <Button key={v.label} size="compact-xs" variant="default" onClick={() => setSettings({ view: v.view })}>
                  {v.label}
                </Button>
              ))}
            </Group>
          </Stack>
        </Stack>
      </div>

      {error && (
        <Alert mt="sm" color="red" icon={<IconAlertTriangle size={16} />} title="エレメント編集でエラーが発生しました">
          {error}
        </Alert>
      )}
      {duplicate && (
        <Alert mt="sm" color="red" icon={<IconAlertTriangle size={16} />}>
          No.{duplicate[0] + 1} と No.{duplicate[1] + 1} はまったく同一のエレメントです。間隔を空けるか、形状を変えてください。
        </Alert>
      )}
      {current && edit && edit.warnings.length > 0 && (
        <Alert mt="sm" color="yellow" icon={<IconAlertTriangle size={16} />} title="反映すると給電点・負荷が変わります">
          {edit.warnings.join('\n')}
        </Alert>
      )}

      <Group justify="space-between" mt="sm" wrap="nowrap">
        <Text size="xs" c="dimmed">
          端どうしがつながったワイヤを 1 つのエレメントとして扱います。「新規」の行に入力すると、最後のエレメントを複製して追加します。
        </Text>
        <Group gap="xs" wrap="nowrap">
          <Button variant="default" onClick={cancel}>
            キャンセル
          </Button>
          <Button onClick={apply} disabled={!current || !!duplicate}>
            反映
          </Button>
        </Group>
      </Group>

      {replaceOpen && edit && (
        <ReplaceDialog
          columns={[...lengthCols, 'radius', 'segmentation']}
          initialColumn={lengthCols.includes(focusCol) || focusCol === 'radius' || focusCol === 'segmentation' ? focusCol : 'width'}
          initialSearch={(col) => {
            if (selected == null || !rows[selected]) return '';
            if (col === 'segmentation') {
              const s = rows[selected].segmentation;
              return s ? formatSegmentation(s) : '';
            }
            const v = cellValue(rows, baseRow, selected, col, units);
            return v == null ? '' : strDbl(v);
          }}
          wavelengthFor={exprWavelength}
          onClose={() => setReplaceOpen(false)}
          onReplace={(col, search, replacement) => {
            const ops = replaceOps(rows, baseRow, col, search, replacement, units, settings.scaleAll);
            push(ops);
            notifications.show({ message: ops.length ? `${ops.length} 件を置換しました` : '一致する値はありません' });
            setReplaceOpen(false);
          }}
        />
      )}
    </Modal>
  );
}

function KindMenu({ label, row, muted, onPick }: { label: string; row: number; muted?: boolean; onPick: (k: ElementKind) => void }) {
  return (
    <Menu position="bottom-start" shadow="md" width={170}>
      <Menu.Target>
        <UnstyledButton className="kind-button" aria-label={`エレメント ${row + 1} の形式: ${label}`} c={muted ? 'dimmed' : undefined}>
          {label}
          <IconChevronDown size={11} />
        </UnstyledButton>
      </Menu.Target>
      <Menu.Dropdown>
        <Menu.Label>エレメント形式の変更</Menu.Label>
        {KIND_MENU.map((group, g) => (
          <Fragment key={g}>
            {g > 0 && <Menu.Divider />}
            {group.map((k) => (
              <Menu.Item key={k} onClick={() => onPick(k)}>
                {KIND_LABELS[k]}
              </Menu.Item>
            ))}
          </Fragment>
        ))}
      </Menu.Dropdown>
    </Menu>
  );
}

interface ReplaceDialogProps {
  columns: ElementColumn[];
  initialColumn: ElementColumn;
  /** The selected row's value in a column, as text. */
  initialSearch: (col: ElementColumn) => string;
  wavelengthFor: (col: ElementColumn) => number;
  onClose: () => void;
  onReplace: (col: ElementColumn, search: number | Segmentation, replacement: number | Segmentation) => void;
}

function ReplaceDialog({ columns, initialColumn, initialSearch, wavelengthFor, onClose, onReplace }: ReplaceDialogProps) {
  const [col, setCol] = useState<ElementColumn>(initialColumn);
  const [search, setSearch] = useState(() => initialSearch(initialColumn));
  const [replacement, setReplacement] = useState('');
  const parse = (t: string) =>
    col === 'segmentation' ? parseSegmentation(t) : evaluate(t, { wavelength: wavelengthFor(col) });
  const s = parse(search);
  const r = parse(replacement);

  return (
    <Modal opened onClose={onClose} title="検索と置換" size="sm">
      <Stack gap="sm">
        <Select
          label="列"
          allowDeselect={false}
          value={col}
          onChange={(v) => {
            if (!v) return;
            setCol(v as ElementColumn);
            setSearch(initialSearch(v as ElementColumn));
          }}
          data={columns.map((c) => ({ value: c, label: COLUMN_LABELS[c] }))}
        />
        <TextInput label="検索する値" value={search} onChange={(e) => setSearch(e.currentTarget.value)} error={search && s == null ? '値を読めません' : null} data-autofocus />
        <TextInput
          label="置換後の値"
          value={replacement}
          onChange={(e) => setReplacement(e.currentTarget.value)}
          error={replacement && r == null ? '値を読めません' : null}
        />
        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={onClose}>
            キャンセル
          </Button>
          <Button disabled={s == null || r == null} onClick={() => s != null && r != null && onReplace(col, s, r)}>
            すべて置換
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
