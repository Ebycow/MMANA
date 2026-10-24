// Tools and selection commands over the 3D view, and the dialog that moves
// or turns the selection by exact amounts (optionally as copies).
import {
  ActionIcon,
  Button,
  Checkbox,
  Divider,
  Group,
  Indicator,
  Menu,
  Modal,
  NumberInput,
  Paper,
  Popover,
  ScrollArea,
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
  IconCircleCheck,
  IconCopy,
  IconFlipVertical,
  IconHexagon,
  IconLayoutAlignCenter,
  IconLine,
  IconLink,
  IconLinkOff,
  IconMagnet,
  IconPlugConnected,
  IconPointer,
  IconRectangle,
  IconRotate,
  IconRotateClockwise2,
  IconTrash,
} from '@tabler/icons-react';
import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { evaluate } from '../domain/expr';
import { describeIssue, findJunctionIssues, fixable, isMistake, issuePoint, issueWires, type JunctionIssue } from '../domain/junctions';
import { wavelength } from '../domain/project';
import { AXES, type Axis } from '../domain/wireEdit';
import { PLANE_MODES, isDrawTool, useEditor, type PlaneMode, type Tool } from '../state/editor';
import { useHeldText, useKeyText } from '../state/input';
import { useApp } from '../state/store';
import {
  alignSelected,
  deleteSelected,
  duplicateSelected,
  fixIssues,
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

export const STEPS = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 5];
export const stepLabel = (m: number) => (m < 0.01 ? `${m * 1000} mm` : m < 1 ? `${Number((m * 100).toFixed(1))} cm` : `${m} m`);

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
        <JunctionCheck editable={editable} />
      </Paper>
      {isDrawTool(tool) && <DrawBar />}
      <SelectionBar editable={editable} />
    </div>
  );
}

function SnapSettings({ autoStep }: { autoStep: number }) {
  const s = useEditor();
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
          <Switch label="端点・中点・ワイヤー上に吸着" checked={s.vertexSnap} onChange={(e) => s.set({ vertexSnap: e.currentTarget.checked })} />
          <Switch label="回転を 15° 刻みにする" checked={s.angleSnap} onChange={(e) => s.set({ angleSnap: e.currentTarget.checked })} />
          <Tooltip label={`動かした端点に接していたほかのワイヤーの端も一緒に動かします（${followKey} を押しながらドラッグで一時的に切り替え）`} multiline w={240}>
            <div>
              <Switch label="つながりを保つ" checked={s.follow} onChange={(e) => s.set({ follow: e.currentTarget.checked })} />
            </div>
          </Tooltip>
          <Tooltip
            label="描いたり動かしたりした端がワイヤーの途中に乗ったら、そのワイヤーをそこで分けて計算上もつなげます（計算では端点どうしが完全に一致しないとつながりません）"
            multiline
            w={240}
          >
            <div>
              <Switch label="途中に付けたら分けてつなぐ" checked={s.autoJoin} onChange={(e) => s.set({ autoJoin: e.currentTarget.checked })} />
            </div>
          </Tooltip>
          <Divider />
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

const FIX_LABEL: Record<JunctionIssue['kind'], string> = { tee: 'つなぐ', gap: 'くっつける', cross: '交点でつなぐ', overlap: '重複を削除' };

/**
 * Lists the places where wires touch but the calculation does not join them,
 * marks them in the view while open and fixes them one by one or all at once.
 */
function JunctionCheck({ editable }: { editable: boolean }) {
  const wires = useApp((s) => s.project.antenna.wires);
  const setSelection = useApp((s) => s.setSelection);
  const set = useEditor((s) => s.set);
  const [open, setOpen] = useState(false);
  const issues = useMemo(() => findJunctionIssues(wires), [wires]);
  const mistakes = issues.filter(isMistake);
  const fixAll = mistakes.filter((i) => fixable(wires, i));
  useEffect(() => set({ issueMarks: open ? issues.map((i) => issuePoint(wires, i)) : [], issueHot: null }), [open, issues, wires, set]);
  return (
    <Popover opened={open} onChange={setOpen} position="bottom-start" shadow="md" withinPortal>
      <Popover.Target>
        <Tooltip label={mistakes.length ? `つながっていない可能性のある箇所が ${mistakes.length} か所あります` : '接続チェック'}>
          <Indicator label={mistakes.length} disabled={mistakes.length === 0} color="red" size={14} offset={4}>
            <ActionIcon
              aria-label="接続チェック"
              variant={open || mistakes.length ? 'light' : 'subtle'}
              color={mistakes.length ? 'red' : 'gray'}
              onClick={() => setOpen((o) => !o)}
            >
              {mistakes.length ? <IconLinkOff size={15} /> : <IconLink size={15} />}
            </ActionIcon>
          </Indicator>
        </Tooltip>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="xs" w={380}>
          <Text size="sm" fw={600}>
            接続チェック
          </Text>
          <Text size="xs" c="dimmed">
            計算では、ワイヤーは端点の座標が完全に一致したときだけつながります。見た目は触れていても計算上は切れている箇所を探します（触れている = 半径の和より近い）。
          </Text>
          {issues.length === 0 ? (
            <Group gap={6} wrap="nowrap">
              <IconCircleCheck size={16} color="var(--mantine-color-teal-6)" />
              <Text size="sm">問題は見つかりませんでした</Text>
            </Group>
          ) : (
            <ScrollArea.Autosize mah={300} type="auto">
              <Stack gap={4}>
                {issues.slice(0, 200).map((issue, i) => (
                  <Group
                    key={i}
                    gap="xs"
                    wrap="nowrap"
                    justify="space-between"
                    className="issue-row"
                    onMouseEnter={() => set({ issueHot: i })}
                    onMouseLeave={() => set({ issueHot: null })}
                  >
                    <Text
                      size="xs"
                      c={isMistake(issue) ? undefined : 'dimmed'}
                      style={{ cursor: 'pointer' }}
                      onClick={() => setSelection(issueWires(issue))}
                      title="クリックでワイヤーを選択"
                    >
                      {describeIssue(issue)}
                    </Text>
                    <Tooltip label="組み合わせワイヤーは分けられません" disabled={fixable(wires, issue)}>
                      <Button size="compact-xs" variant="light" disabled={!editable || !fixable(wires, issue)} onClick={() => fixIssues([issue])}>
                        {FIX_LABEL[issue.kind]}
                      </Button>
                    </Tooltip>
                  </Group>
                ))}
              </Stack>
            </ScrollArea.Autosize>
          )}
          {fixAll.length > 0 && (
            <Button size="xs" color="red" disabled={!editable} onClick={() => fixIssues(fixAll)}>
              すべて直す（{fixAll.length} か所）
            </Button>
          )}
          {issues.length > mistakes.length && (
            <Text size="xs" c="dimmed">
              交差は「すべて直す」に含めません。たすき掛けの位相線（HB9CV・LPDA など）は交差させてつながない書き方が正しいためです。
            </Text>
          )}
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

const PLANE_LABEL: Record<PlaneMode, string> = { auto: '自動', xy: 'XY', xz: 'XZ', yz: 'YZ' };

/** The drawing plane, in reach while a drawing tool is on. */
function DrawBar() {
  const plane = useEditor((s) => s.plane);
  const setPlane = useEditor((s) => s.setPlane);
  // the plane in use: fixed by a shape's first click, else the one auto picked last
  const current = useEditor((s) => s.draftPlane ?? s.cursor?.plane ?? null);
  const constrainKey = useHeldText('constrain');
  const key = useKeyText('drawPlane');
  return (
    <Paper className="view3d-toolbar-row" shadow="xs" withBorder>
      <Tooltip
        multiline
        w={260}
        label={`点は直前の点を通るこの面の上に置きます。自動は視線に最も向いた面です。${key ? `${key} で切り替え。` : ''}${constrainKey} で 15° 刻み`}
      >
        <Text size="xs" px={6} style={{ whiteSpace: 'nowrap' }}>
          作図面
        </Text>
      </Tooltip>
      <SegmentedControl
        size="xs"
        aria-label="作図面"
        value={plane}
        onChange={(v) => setPlane(v as PlaneMode)}
        data={PLANE_MODES.map((m) => ({
          value: m,
          label: m === 'auto' && plane === 'auto' && current ? `自動（${PLANE_LABEL[current]}）` : PLANE_LABEL[m],
        }))}
      />
    </Paper>
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
