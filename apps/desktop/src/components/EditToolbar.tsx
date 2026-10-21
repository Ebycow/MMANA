// Tools and selection commands over the 3D view, and the dialog that moves
// or turns the selection by exact amounts (optionally as copies).
import {
  ActionIcon,
  Button,
  Checkbox,
  Divider,
  Group,
  Menu,
  Modal,
  NumberInput,
  Paper,
  Popover,
  SegmentedControl,
  Select,
  Stack,
  Switch,
  Text,
  TextInput,
  Tooltip,
} from '@mantine/core';
import {
  IconAdjustments,
  IconArrowsMove,
  IconCopy,
  IconFlipVertical,
  IconHexagon,
  IconLayoutAlignCenter,
  IconLine,
  IconMagnet,
  IconPlugConnected,
  IconPointer,
  IconRectangle,
  IconRotate,
  IconRotateClockwise2,
  IconTrash,
} from '@tabler/icons-react';
import { useState, type ReactNode } from 'react';
import { evaluate } from '../domain/expr';
import { wavelength } from '../domain/project';
import { AXES, type Axis } from '../domain/wireEdit';
import { useEditor, type PlaneMode, type Tool } from '../state/editor';
import { useHeldText, useKeyText } from '../state/input';
import { useApp } from '../state/store';
import {
  alignSelected,
  deleteSelected,
  duplicateSelected,
  flipSelected,
  mirrorCopySelected,
  rotateSelected,
  transformSelected,
} from '../state/wireCommands';
import { KeyHint } from './InputSettings';

/** The keys held for selecting, as written in the help. */
type Held = { add: string; box: string };

const TOOLS: { tool: Tool; label: string; help: (h: Held) => string; icon: ReactNode }[] = [
  {
    tool: 'select',
    label: '選択',
    help: (h) => `クリックで選択、${h.add} で追加、${h.box} + ドラッグで範囲選択、ダブルクリックでつながったワイヤー。ワイヤーや端点はそのままドラッグで動かせます`,
    icon: <IconPointer size={15} />,
  },
  { tool: 'move', label: '移動', help: () => '矢印・平面のハンドルをドラッグして、選択を軸に沿って動かします', icon: <IconArrowsMove size={15} /> },
  { tool: 'rotate', label: '回転', help: () => 'リングをドラッグして、選択をその中心で回します', icon: <IconRotate size={15} /> },
  { tool: 'wire', label: 'ワイヤーを描く', help: () => 'クリックで点を打ってつなげます。始点をクリックで閉じたループ。数字を打つと長さを指定。Enter・Esc・右クリックで終了', icon: <IconLine size={15} /> },
  { tool: 'rect', label: '四角ループ', help: () => '対角の 2 点をクリックして、作図面の上に四角形のループを描きます', icon: <IconRectangle size={15} /> },
  { tool: 'polygon', label: '多角形ループ', help: () => '中心と 1 つの頂点をクリックして、正多角形のループを描きます。数字を打つと半径を指定', icon: <IconHexagon size={15} /> },
  {
    tool: 'feed',
    label: '給電点・負荷を移動',
    help: () => '給電点（球）や負荷（箱）をドラッグして、ワイヤー上のパルスに置き直します。位置は w1b3 のような MMANA のパルス名で保存されます',
    icon: <IconPlugConnected size={15} />,
  },
];

/** Tools that start a new group in the toolbar. */
const GROUP_START: Tool[] = ['wire', 'feed'];

const STEPS = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 5];
const stepLabel = (m: number) => (m < 0.01 ? `${m * 1000} mm` : m < 1 ? `${Number((m * 100).toFixed(1))} cm` : `${m} m`);

export function EditToolbar({ autoStep, editable }: { autoStep: number; editable: boolean }) {
  const tool = useEditor((s) => s.tool);
  const setTool = useEditor((s) => s.setTool);
  const held = { add: useHeldText('addToSelection'), box: useHeldText('boxSelect') };
  return (
    <div className="view3d-toolbar">
      <Paper className="view3d-toolbar-row" shadow="xs" withBorder>
        <ActionIcon.Group>
          {TOOLS.map((t, i) => (
            <Tooltip
              key={t.tool}
              multiline
              w={260}
              openDelay={300}
              label={
                <>
                  <Group gap={6} justify="space-between" wrap="nowrap">
                    <Text size="xs" fw={600}>
                      {t.label}
                    </Text>
                    <KeyHint id={`tool.${t.tool}`} />
                  </Group>
                  <Text size="xs">{t.help(held)}</Text>
                </>
              }
            >
              <ActionIcon
                aria-label={t.label}
                variant={tool === t.tool ? 'filled' : 'subtle'}
                color={tool === t.tool ? undefined : 'gray'}
                disabled={!editable && i > 0}
                onClick={() => setTool(t.tool)}
                style={GROUP_START.includes(t.tool) ? { marginLeft: 6 } : undefined}
              >
                {t.icon}
              </ActionIcon>
            </Tooltip>
          ))}
        </ActionIcon.Group>
        <Divider orientation="vertical" mx={4} />
        <SnapSettings autoStep={autoStep} />
      </Paper>
      <SelectionBar editable={editable} />
    </div>
  );
}

function SnapSettings({ autoStep }: { autoStep: number }) {
  const s = useEditor();
  const constrainKey = useHeldText('constrain');
  const followKey = useHeldText('followToggle');
  const stepValue = s.step == null ? 'auto' : String(s.step);
  return (
    <Popover position="bottom-start" shadow="md" withinPortal>
      <Popover.Target>
        <Tooltip label="スナップと作図の設定">
          <ActionIcon aria-label="スナップと作図の設定" variant={s.gridSnap || s.vertexSnap ? 'light' : 'subtle'} color={s.gridSnap || s.vertexSnap ? undefined : 'gray'}>
            <IconMagnet size={15} />
          </ActionIcon>
        </Tooltip>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="xs" w={240}>
          <Group gap="xs" wrap="nowrap" justify="space-between">
            <Switch label="グリッド" checked={s.gridSnap} onChange={(e) => s.set({ gridSnap: e.currentTarget.checked })} />
            <Select
              aria-label="グリッドの間隔"
              w={120}
              size="xs"
              allowDeselect={false}
              value={stepValue}
              data={[{ value: 'auto', label: `自動 (${stepLabel(autoStep)})` }, ...STEPS.map((m) => ({ value: String(m), label: stepLabel(m) }))]}
              onChange={(v) => s.set({ step: v == null || v === 'auto' ? null : Number(v) })}
              comboboxProps={{ withinPortal: false }}
            />
          </Group>
          <Switch label="端点・中点に吸着" checked={s.vertexSnap} onChange={(e) => s.set({ vertexSnap: e.currentTarget.checked })} />
          <Switch label="回転を 15° 刻みにする" checked={s.angleSnap} onChange={(e) => s.set({ angleSnap: e.currentTarget.checked })} />
          <Tooltip label={`動かした端点に接していたほかのワイヤーの端も一緒に動かします（${followKey} を押しながらドラッグで一時的に切り替え）`} multiline w={240}>
            <div>
              <Switch label="つながりを保つ" checked={s.follow} onChange={(e) => s.set({ follow: e.currentTarget.checked })} />
            </div>
          </Tooltip>
          <Divider />
          <div>
            <Text size="xs" c="dimmed" mb={4}>
              作図面（描画ツール）
            </Text>
            <SegmentedControl
              size="xs"
              fullWidth
              value={s.plane}
              onChange={(v) => s.set({ plane: v as PlaneMode })}
              data={[
                { value: 'auto', label: '自動' },
                { value: 'xy', label: 'XY' },
                { value: 'xz', label: 'XZ' },
                { value: 'yz', label: 'YZ' },
              ]}
            />
            <Text size="xs" c="dimmed" mt={4}>
              自動は視線に最も向いた面。直前の点を通る面に描きます。{constrainKey} で 15° 刻み。
            </Text>
          </div>
          <NumberInput
            size="xs"
            label="多角形の角の数"
            min={3}
            max={64}
            value={s.sides}
            onChange={(v) => typeof v === 'number' && v >= 3 && s.set({ sides: Math.round(v) })}
          />
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

function SelectionBar({ editable }: { editable: boolean }) {
  const count = useApp((s) => s.selectedWires.length);
  const open = useEditor((s) => s.transformOpen);
  const set = useEditor((s) => s.set);
  const duplicateKey = useKeyText('duplicate');
  const deleteKey = useKeyText('delete');
  if (count === 0) return null;
  const axisItems = (f: (a: Axis) => void, label: (a: string) => string) =>
    AXES.map((a) => (
      <Menu.Item key={a} onClick={() => f(a)}>
        {label(a.toUpperCase())}
      </Menu.Item>
    ));
  return (
    <Paper className="view3d-toolbar-row" shadow="xs" withBorder>
      <Text size="xs" px={6} className="num" style={{ whiteSpace: 'nowrap' }}>
        {count} 本を選択
      </Text>
      <Tooltip label="数値で移動・回転（複製も）">
        <ActionIcon aria-label="数値で移動・回転" variant="subtle" color="gray" disabled={!editable} onClick={() => set({ transformOpen: true })}>
          <IconAdjustments size={15} />
        </ActionIcon>
      </Tooltip>
      <Tooltip label={duplicateKey ? `その場に複製 (${duplicateKey})` : 'その場に複製'}>
        <ActionIcon aria-label="複製" variant="subtle" color="gray" disabled={!editable} onClick={duplicateSelected}>
          <IconCopy size={15} />
        </ActionIcon>
      </Tooltip>
      <Menu position="bottom-start" withinPortal disabled={!editable}>
        <Menu.Target>
          <Tooltip label="反転・鏡像">
            <ActionIcon aria-label="反転・鏡像" variant="subtle" color="gray" disabled={!editable}>
              <IconFlipVertical size={15} />
            </ActionIcon>
          </Tooltip>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Label>選択の中心で反転</Menu.Label>
          {axisItems(flipSelected, (a) => `${a} 方向に反転`)}
          <Menu.Label>鏡像コピーを追加</Menu.Label>
          {axisItems(mirrorCopySelected, (a) => `${a} = 0 の面に対して`)}
        </Menu.Dropdown>
      </Menu>
      <Menu position="bottom-start" withinPortal disabled={!editable}>
        <Menu.Target>
          <Tooltip label="原点にそろえる">
            <ActionIcon aria-label="原点にそろえる" variant="subtle" color="gray" disabled={!editable}>
              <IconLayoutAlignCenter size={15} />
            </ActionIcon>
          </Tooltip>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Label>選択の中心を</Menu.Label>
          {axisItems(alignSelected, (a) => `${a} = 0 へ`)}
        </Menu.Dropdown>
      </Menu>
      <Menu position="bottom-start" withinPortal disabled={!editable}>
        <Menu.Target>
          <Tooltip label="90° 回転">
            <ActionIcon aria-label="90° 回転" variant="subtle" color="gray" disabled={!editable}>
              <IconRotateClockwise2 size={15} />
            </ActionIcon>
          </Tooltip>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Label>選択の中心で 90° 回転</Menu.Label>
          {AXES.flatMap((a) => [
            <Menu.Item key={`${a}+`} onClick={() => rotateSelected(a, 90)}>
              {a.toUpperCase()} 軸 +90°
            </Menu.Item>,
            <Menu.Item key={`${a}-`} onClick={() => rotateSelected(a, -90)}>
              {a.toUpperCase()} 軸 −90°
            </Menu.Item>,
          ])}
        </Menu.Dropdown>
      </Menu>
      <Tooltip label={deleteKey ? `削除 (${deleteKey})` : '削除'}>
        <ActionIcon aria-label="選択したワイヤーを削除" variant="subtle" color="red" disabled={!editable} onClick={deleteSelected}>
          <IconTrash size={15} />
        </ActionIcon>
      </Tooltip>
      {open && <TransformDialog onClose={() => set({ transformOpen: false })} />}
    </Paper>
  );
}

/** A length field that accepts MMANA expressions (R = wavelength). */
function ExprInput({ label, value, onChange, lambda, unit = 'm' }: { label: string; value: string; onChange: (v: string) => void; lambda: number; unit?: string }) {
  const bad = value.trim() !== '' && evaluate(value, { wavelength: lambda }) == null;
  return (
    <TextInput
      label={label}
      value={value}
      onChange={(e) => onChange(e.currentTarget.value)}
      error={bad ? '数値または式' : undefined}
      rightSection={<Text size="xs" c="dimmed">{unit}</Text>}
      classNames={{ input: 'num' }}
      data-autofocus={label.startsWith('ΔX') || undefined}
    />
  );
}

function TransformDialog({ onClose }: { onClose: () => void }) {
  const lambda = wavelength(useApp((s) => s.project.antenna.frequencyMhz));
  const [kind, setKind] = useState<'move' | 'rotate'>('move');
  const [d, setD] = useState({ x: '0', y: '0', z: '0' });
  const [axis, setAxis] = useState<Axis>('z');
  const [deg, setDeg] = useState('90');
  const [about, setAbout] = useState<'selection' | 'origin'>('selection');
  const [copy, setCopy] = useState(false);
  const [copies, setCopies] = useState(1);

  const num = (t: string) => (t.trim() === '' ? 0 : evaluate(t, { wavelength: lambda }));
  const offset = { x: num(d.x), y: num(d.y), z: num(d.z) };
  const angle = num(deg);
  const valid = kind === 'move' ? offset.x != null && offset.y != null && offset.z != null : angle != null;

  const apply = () => {
    if (!valid) return;
    const n = copy ? copies : 0;
    if (kind === 'move') transformSelected({ kind: 'move', offset: { x: offset.x!, y: offset.y!, z: offset.z! }, copies: n });
    else transformSelected({ kind: 'rotate', axis, deg: angle!, about, copies: n });
    onClose();
  };

  return (
    <Modal opened onClose={onClose} title="数値で移動・回転" size="sm" centered>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          apply();
        }}
      >
        <Stack gap="sm">
          <SegmentedControl
            value={kind}
            onChange={(v) => setKind(v as 'move' | 'rotate')}
            data={[
              { value: 'move', label: '移動' },
              { value: 'rotate', label: '回転' },
            ]}
          />
          {kind === 'move' ? (
            <Group grow gap="xs">
              {AXES.map((a) => (
                <ExprInput key={a} label={`Δ${a.toUpperCase()}`} value={d[a]} lambda={lambda} onChange={(v) => setD({ ...d, [a]: v })} />
              ))}
            </Group>
          ) : (
            <>
              <Group gap="xs" align="end" grow>
                <div>
                  <Text size="sm" fw={500} mb={4}>
                    軸
                  </Text>
                  <SegmentedControl fullWidth value={axis} onChange={(v) => setAxis(v as Axis)} data={AXES.map((a) => ({ value: a, label: a.toUpperCase() }))} />
                </div>
                <ExprInput label="角度" value={deg} lambda={lambda} unit="°" onChange={setDeg} />
              </Group>
              <SegmentedControl
                size="xs"
                value={about}
                onChange={(v) => setAbout(v as 'selection' | 'origin')}
                data={[
                  { value: 'selection', label: '選択の中心で' },
                  { value: 'origin', label: '原点を通る軸で' },
                ]}
              />
            </>
          )}
          <Group gap="xs" align="center">
            <Checkbox label="コピーを作る" checked={copy} onChange={(e) => setCopy(e.currentTarget.checked)} />
            <NumberInput size="xs" w={90} min={1} max={200} disabled={!copy} value={copies} onChange={(v) => typeof v === 'number' && setCopies(Math.max(1, Math.round(v)))} suffix=" 個" />
          </Group>
          <Text size="xs" c="dimmed">
            {copy
              ? `選択はそのままに、${kind === 'move' ? '移動量' : '角度'}ずつずらしたコピーを ${copies} 個追加します（導波器の並びやラジアルなど）。`
              : `R は波長 (${Number(lambda.toFixed(3))} m)。式も使えます（例: R/4）。`}
          </Text>
          <Group justify="flex-end" gap="xs">
            <Button variant="default" onClick={onClose}>
              キャンセル
            </Button>
            <Button type="submit" disabled={!valid}>
              適用
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}
