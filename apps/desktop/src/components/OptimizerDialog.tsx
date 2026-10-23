// 最適化 (MMANA's ACalBox dialog and the run): the variables and goals of the
// optimisation sheet, the goal details (ACalInfo), the bands (ACalMult) and
// the run with its log. The setup edits and the search run in mmana-core
// (`optimize.rs`); the dialog sends the sheet and shows what comes back.
import {
  ActionIcon,
  Alert,
  Badge,
  Button,
  Checkbox,
  Group,
  Loader,
  Menu,
  Modal,
  NumberInput,
  Radio,
  ScrollArea,
  SegmentedControl,
  Slider,
  Stack,
  Table,
  Text,
  TextInput,
  Tooltip,
} from '@mantine/core';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import {
  IconAlertTriangle,
  IconChevronDown,
  IconLink,
  IconListDetails,
  IconPlayerPlay,
  IconPlayerStop,
  IconPlus,
  IconRowInsertTop,
  IconTrash,
  IconWand,
} from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';
import type {
  Band,
  ElementAxis,
  ElementCombination,
  ElementRow,
  Goals,
  OptimizerSetup,
  Progress,
  Project,
  SetupEdit,
  SetupOp,
  Variable,
  VariableTarget,
} from '../api/types';
import { EngineError, transport } from '../api/transport';
import { KIND_LABELS, strDbl } from '../domain/elements';
import {
  TARGET_KINDS,
  WEIGHT_LABELS,
  formatBound,
  formatLink,
  issueMessage,
  kindLabel,
  makeTarget,
  matchingJx,
  needsCombination,
  parseBound,
  parseLink,
  partIndex,
  partLabels,
  targetPosition,
  targetUnit,
  tradeOffText,
  withBound,
  type PartContext,
  type TargetKind,
} from '../domain/optimize';
import { recordError } from '../state/errorLog';
import { useApp } from '../state/store';
import { NumCell, TextCell } from './Cells';

function reportError(e: unknown) {
  const err = e instanceof EngineError ? e : new EngineError('internal', 'エラー', String(e));
  notifications.show({ color: 'red', title: err.title, message: err.detail });
  recordError(err.title, err.detail);
}

/** The 箇所 labels need to know whether an element's perimeter is an X width, and a load's kind. */
function partContext(t: VariableTarget, project: Project, elements: ElementRow[]): PartContext {
  if (t.kind === 'element') return { xWidth: elements[t.element]?.xWidth };
  if (t.kind === 'load') return { loadKind: project.antenna.loads[t.load]?.kind };
  return {};
}

/** "エレメント 2 横幅" */
export function describeTarget(t: VariableTarget, project: Project, elements: ElementRow[]): string {
  const pos = targetPosition(t);
  const parts = partLabels(t.kind, partContext(t, project, elements));
  return [kindLabel(t.kind), pos != null ? pos + 1 : null, parts[partIndex(t)]].filter((x) => x != null).join(' ');
}

// the element editor's 間隔表記 and 全ての座標を同一比率で変更, which "all elements" follows like MMANA
function elementEditorFlags(): { intervals: boolean; scaleAll: boolean } {
  try {
    const s = JSON.parse(localStorage.getItem('mmana.element-editor') ?? '{}') as { intervals?: boolean; scaleAll?: boolean };
    return { intervals: s.intervals ?? true, scaleAll: s.scaleAll ?? false };
  } catch {
    return { intervals: true, scaleAll: false };
  }
}

export function OptimizerDialog() {
  const open = useApp((s) => s.optimizer.dialogOpen);
  return open ? <OptimizerDialogBody /> : null;
}

function OptimizerDialogBody() {
  const project = useApp((s) => s.project);
  const options = useApp((s) => s.options);
  const opt = useApp((s) => s.optimizer);
  const setOptimizer = useApp((s) => s.setOptimizer);
  const setup = opt.setup;
  const [info, setInfo] = useState<SetupEdit | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [view, setView] = useState<'setup' | 'run'>(opt.running ? 'run' : 'setup');
  const [dialog, setDialog] = useState<null | 'goals' | 'bands' | 'allElements' | 'pitch'>(null);
  // remounts the cells after an edit so they show the values that came back
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    let live = true;
    transport.editOptimizerSetup({ project, options, setup, operations: [] }).then(
      (e) => live && setInfo(e),
      (e: unknown) => live && reportError(e),
    );
    return () => {
      live = false;
    };
  }, [project, options, setup]);

  const close = () => setOptimizer({ dialogOpen: false });
  const setSetup = (s: OptimizerSetup) => setOptimizer({ setup: s });
  const runOps = async (operations: SetupOp[]) => {
    try {
      const e = await transport.editOptimizerSetup({ project, options, setup, operations });
      if (e.applied.includes(false)) notifications.show({ color: 'yellow', message: 'この操作はできません' });
      setSetup(e.setup);
      setInfo(e);
      setRevision((r) => r + 1);
    } catch (e) {
      reportError(e);
    }
  };
  const patchVariable = (i: number, v: Variable) => {
    setSetup({ ...setup, variables: setup.variables.map((x, k) => (k === i ? v : x)) });
    setRevision((r) => r + 1);
  };

  const run = async () => {
    setView('run');
    await useApp.getState().runOptimization();
    const o = useApp.getState().optimizer;
    if (o.outcome?.best && !o.error) {
      notifications.show({ message: '最適化の結果をアンテナに反映しました。「元に戻す」で最適化前に戻せます。' });
    }
  };

  const running = opt.running;
  return (
    <Modal
      opened
      onClose={() => !running && close()}
      closeOnEscape={!running}
      withCloseButton={!running}
      size="min(1320px, 96vw)"
      title={
        <Text fw={600}>
          最適化
          <Text span c="dimmed" ml="xs" size="sm">
            {project.antenna.name}
          </Text>
        </Text>
      }
    >
      {view === 'run' ? (
        <RunView project={project} elements={info?.elements ?? []} onBack={() => setView('setup')} onClose={close} />
      ) : (
        <SetupView
          project={project}
          setup={setup}
          info={info}
          selected={selected}
          revision={revision}
          onSelect={setSelected}
          onSetup={setSetup}
          onOps={runOps}
          onVariable={patchVariable}
          onDialog={setDialog}
          onRun={run}
          onClose={close}
        />
      )}
      {dialog === 'goals' && (
        <GoalsDialog
          goals={setup.goals}
          project={project}
          onClose={() => setDialog(null)}
          onApply={(goals) => setSetup({ ...setup, goals })}
        />
      )}
      {dialog === 'bands' && (
        <BandsDialog setup={setup} project={project} onClose={() => setDialog(null)} onApply={(patch) => setSetup({ ...setup, ...patch })} />
      )}
      {dialog === 'allElements' && (
        <AllElementsDialog
          ask={needsCombination(info?.elements ?? [])}
          replaces={setup.variables.length > 0}
          onClose={() => setDialog(null)}
          onApply={(op) => runOps([op])}
        />
      )}
      {dialog === 'pitch' && (
        <PitchDialog
          initial={setup.variables[selected ?? 0]?.pitch ?? 0.01}
          percent={!setup.absolutePitch}
          onClose={() => setDialog(null)}
          onApply={(pitch) =>
            setSetup({ ...setup, variables: setup.variables.map((v) => (!v.link || v.link.kind === 'independent' ? { ...v, pitch } : v)) })
          }
        />
      )}
    </Modal>
  );
}

// ---------------------------------------------------------------- setup ---

interface SetupViewProps {
  project: Project;
  setup: OptimizerSetup;
  info: SetupEdit | null;
  selected: number | null;
  revision: number;
  onSelect: (i: number | null) => void;
  onSetup: (s: OptimizerSetup) => void;
  onOps: (ops: SetupOp[]) => void;
  onVariable: (i: number, v: Variable) => void;
  onDialog: (d: 'goals' | 'bands' | 'allElements' | 'pitch') => void;
  onRun: () => void;
  onClose: () => void;
}

function SetupView({ project, setup, info, selected, revision, onSelect, onSetup, onOps, onVariable, onDialog, onRun, onClose }: SetupViewProps) {
  const outcome = useApp((s) => s.optimizer.outcome);
  const setOptimizer = useApp((s) => s.setOptimizer);
  const elements = info?.elements ?? [];
  const values = info?.values ?? [];
  const ant = project.antenna;
  const stacked = ant.stack.horizontal > 1 || ant.stack.vertical > 1;
  const bandsUsable = !setup.sweepOnly && ant.sources.length <= 4;
  const sel = selected != null ? setup.variables[selected] : undefined;
  const canLinkSame = sel != null && (sel.target.kind === 'wire' || sel.target.kind === 'load');

  const positions = (kind: TargetKind): { value: number; label: string }[] => {
    switch (kind) {
      case 'wire':
      case 'polarWire':
        return ant.wires.map((_, i) => ({ value: i, label: `${i + 1}` }));
      case 'element':
        return elements.map((r, i) => ({ value: i, label: `${i + 1} ${KIND_LABELS[r.kind]}` }));
      case 'load':
        return ant.loads.map((l, i) => ({ value: i, label: `${i + 1} ${l.pulse}` }));
      case 'source':
        return ant.sources.map((s, i) => ({ value: i, label: `${i + 1} ${s.pulse}` }));
      default:
        return [];
    }
  };
  const newTarget: VariableTarget = elements.length > 0 ? { kind: 'element', element: 0, part: 'width' } : { kind: 'wire', wire: 0, part: 'y1' };

  return (
    <>
      <Group gap="xs" mb="xs" wrap="wrap">
        <Button size="xs" leftSection={<IconPlus size={14} />} onClick={() => onOps([{ op: 'setTarget', index: setup.variables.length, target: newTarget }])}>
          変数を追加
        </Button>
        <Tooltip label="すべてのエレメントの幅・全周と間隔（または位置）を、ラジエータ・リフレクタ・ディレクタの順に登録します" multiline w={300}>
          <Button size="xs" variant="default" leftSection={<IconWand size={14} />} disabled={elements.length === 0} onClick={() => onDialog('allElements')}>
            全エレメント
          </Button>
        </Tooltip>
        <Menu position="bottom-start" shadow="md" width={250}>
          <Menu.Target>
            <Button size="xs" variant="default" rightSection={<IconChevronDown size={12} />}>
              変数の操作
            </Button>
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Item leftSection={<IconLink size={14} />} disabled={!canLinkSame} onClick={() => selected != null && onOps([{ op: 'linkSame', index: selected }])}>
              自動連携登録（選択中のワイヤ・集中定数）
            </Menu.Item>
            <Menu.Item leftSection={<IconLink size={14} />} disabled={setup.variables.length === 0} onClick={() => onOps([{ op: 'linkElements' }])}>
              エレメントの連携設定
            </Menu.Item>
            <Menu.Item disabled={setup.variables.length === 0} onClick={() => onDialog('pitch')}>
              ピッチの一括設定…
            </Menu.Item>
            <Menu.Divider />
            <Menu.Item
              color="red"
              leftSection={<IconTrash size={14} />}
              disabled={setup.variables.length === 0}
              onClick={() =>
                modals.openConfirmModal({
                  title: '変数をすべて削除しますか？',
                  labels: { confirm: 'すべて削除', cancel: 'キャンセル' },
                  confirmProps: { color: 'red' },
                  onConfirm: () => onOps([{ op: 'clear' }]),
                })
              }
            >
              全て削除
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
        <div style={{ flex: 1 }} />
        <Checkbox
          size="xs"
          label="可変ピッチは絶対値"
          checked={setup.absolutePitch}
          onChange={(e) => onSetup({ ...setup, absolutePitch: e.currentTarget.checked })}
        />
        <Tooltip label="パターンを 2° ごとに計算します（約 2 倍速。周波数が高く大地がある場合は精度に注意）" multiline w={260}>
          <Checkbox size="xs" label="分解能 2°" checked={setup.coarsePattern} onChange={(e) => onSetup({ ...setup, coarsePattern: e.currentTarget.checked })} />
        </Tooltip>
      </Group>

      <div style={{ display: 'flex', gap: 12, height: '56vh', minHeight: 320 }}>
        <ScrollArea style={{ flex: 1, minWidth: 0, border: '1px solid var(--panel-border)', borderRadius: 8 }} type="auto">
          <table className="grid-table" aria-label="可変パラメータ">
            <thead>
              <tr>
                <th style={{ textAlign: 'center' }}>No.</th>
                <th className="left">種類</th>
                <th className="left" style={{ minWidth: 120 }}>
                  位置
                </th>
                <th className="left">箇所</th>
                <th>
                  <Tooltip label="0 = 独立変数。n で変数 n と同じ値、-n でその負値。n+c、-n+c、n*c、n/c の式も書けます（c に = と書くと現在値から求めます）" multiline w={300}>
                    <span>連携</span>
                  </Tooltip>
                </th>
                <th>{setup.absolutePitch ? 'ピッチ' : 'ピッチ (%)'}</th>
                <th>
                  <Tooltip label="数値、または #n で変数 n の値" withArrow>
                    <span>Min</span>
                  </Tooltip>
                </th>
                <th>Max</th>
                <th>現在値</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {setup.variables.map((v, i) => {
                const t = v.target;
                const pos = targetPosition(t);
                const ctx = partContext(t, project, elements);
                const parts = partLabels(t.kind, ctx);
                const independent = !v.link || v.link.kind === 'independent';
                const value = values[i];
                const linked = info?.linkedValues[i];
                const differs = linked != null && value != null && Number(linked.toFixed(6)) !== Number(value.toFixed(6));
                const setTarget = (target: VariableTarget) => onOps([{ op: 'setTarget', index: i, target }]);
                return (
                  <tr key={`${i}-${revision}`} className={selected === i ? 'selected' : undefined} onFocus={() => onSelect(i)}>
                    <td className="index num" onClick={() => onSelect(selected === i ? null : i)}>
                      {i + 1}
                    </td>
                    <td className="left">
                      <select
                        className="cell-select"
                        aria-label={`変数 ${i + 1} の種類`}
                        value={t.kind}
                        onChange={(e) => setTarget(makeTarget(e.currentTarget.value as TargetKind, pos ?? 0, partIndex(t)))}
                      >
                        {TARGET_KINDS.map((k) => (
                          <option key={k.kind} value={k.kind} disabled={k.kind === 'stack' && !stacked && t.kind !== 'stack'}>
                            {k.label}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td className="left">
                      {pos == null ? (
                        <Text size="xs" c="dimmed" px={6}>
                          —
                        </Text>
                      ) : (
                        <select
                          className="cell-select"
                          aria-label={`変数 ${i + 1} の位置`}
                          value={pos}
                          onChange={(e) => setTarget(makeTarget(t.kind, Number(e.currentTarget.value), partIndex(t)))}
                        >
                          {positions(t.kind).map((p) => (
                            <option key={p.value} value={p.value}>
                              {p.label}
                            </option>
                          ))}
                          {pos >= positions(t.kind).length && <option value={pos}>{pos + 1}（なし）</option>}
                        </select>
                      )}
                    </td>
                    <td className="left">
                      {parts.length === 0 ? (
                        <Text size="xs" c="dimmed" px={6}>
                          —
                        </Text>
                      ) : (
                        <select
                          className="cell-select"
                          aria-label={`変数 ${i + 1} の箇所`}
                          value={partIndex(t)}
                          onChange={(e) => setTarget(makeTarget(t.kind, pos ?? 0, Number(e.currentTarget.value)))}
                        >
                          {parts.map((label, k) => (
                            <option key={k} value={k}>
                              {label}
                            </option>
                          ))}
                        </select>
                      )}
                    </td>
                    <td style={{ minWidth: 70 }}>
                      <TextCell
                        ariaLabel={`変数 ${i + 1} の連携`}
                        align="right"
                        value={formatLink(v.link)}
                        validate={(text) => parseLink(text, i, values) != null}
                        onCommit={(text) => onVariable(i, { ...v, link: parseLink(text, i, values)! })}
                      />
                    </td>
                    {independent ? (
                      <>
                        <td style={{ minWidth: 70 }}>
                          <NumCell ariaLabel={`変数 ${i + 1} のピッチ`} value={v.pitch} digits={6} onCommit={(pitch) => onVariable(i, { ...v, pitch })} />
                        </td>
                        {(['min', 'max'] as const).map((which) => (
                          <td key={which} style={{ minWidth: 70 }}>
                            <TextCell
                              ariaLabel={`変数 ${i + 1} の ${which === 'min' ? '最小値' : '最大値'}`}
                              align="right"
                              value={formatBound(v, which)}
                              validate={(text) => parseBound(text) != null}
                              onCommit={(text) => onVariable(i, withBound(v, which, parseBound(text)!))}
                            />
                          </td>
                        ))}
                      </>
                    ) : (
                      <>
                        <td className="readonly" />
                        <td className="readonly" />
                        <td className="readonly" />
                      </>
                    )}
                    <td className="readonly num" style={{ whiteSpace: 'nowrap' }}>
                      {value == null ? '—' : strDbl(value)}
                      {differs && (
                        <Text span c="orange" size="xs" title="連携で設定される値">
                          {' '}
                          / {strDbl(linked)}
                        </Text>
                      )}{' '}
                      <Text span c="dimmed" size="xs">
                        {targetUnit(t, ctx.loadKind)}
                      </Text>
                    </td>
                    <td style={{ width: 52 }}>
                      <Group gap={0} wrap="nowrap" className="row-actions">
                        <Tooltip label="この行に挿入（この変数を複製）">
                          <ActionIcon size="sm" aria-label={`変数 ${i + 1} の前に挿入`} onClick={() => onOps([{ op: 'insert', index: i }])}>
                            <IconRowInsertTop size={14} />
                          </ActionIcon>
                        </Tooltip>
                        <Tooltip label="この行を削除">
                          <ActionIcon
                            size="sm"
                            color="red"
                            aria-label={`変数 ${i + 1} を削除`}
                            onClick={() => {
                              onOps([{ op: 'delete', index: i }]);
                              onSelect(null);
                            }}
                          >
                            <IconTrash size={14} />
                          </ActionIcon>
                        </Tooltip>
                      </Group>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {setup.variables.length === 0 && (
            <Text size="sm" c="dimmed" ta="center" mt="xl" px="md">
              「変数を追加」で可変パラメータ（ワイヤの座標、エレメントの寸法、集中定数、地上高、周波数、給電点、スタック間隔）を登録するか、「全エレメント」で八木などのエレメントをまとめて登録します。
            </Text>
          )}
        </ScrollArea>

        <Stack gap="xs" style={{ width: 280, flexShrink: 0, border: '1px solid var(--panel-border)', borderRadius: 8, padding: 12 }}>
          <Text size="sm" fw={600}>
            評価の割合
          </Text>
          {WEIGHT_LABELS.map(({ key, label }) => (
            <Group key={key} gap="xs" wrap="nowrap">
              <Text size="xs" w={86} c={setup.sweepOnly ? 'dimmed' : undefined}>
                {label}
              </Text>
              <Slider
                style={{ flex: 1 }}
                size="sm"
                min={0}
                max={100}
                disabled={setup.sweepOnly}
                aria-label={`${label} の割合`}
                value={setup.weights[key]}
                onChange={(n) => onSetup({ ...setup, weights: { ...setup.weights, [key]: n } })}
              />
              <Text size="xs" w={26} ta="right" className="num" c="dimmed">
                {setup.weights[key]}
              </Text>
            </Group>
          ))}
          <Text size="xs" c="dimmed">
            {tradeOffText(setup)}
          </Text>
          <Tooltip label="評価せず、各変数を現在値から最大値まで、続いて最小値までピッチごとに動かして結果を記録します" multiline w={260}>
            <Checkbox size="xs" label="目標なし（単純スイープ）" checked={setup.sweepOnly} onChange={(e) => onSetup({ ...setup, sweepOnly: e.currentTarget.checked })} />
          </Tooltip>
          <Group gap="xs" grow>
            <Button size="xs" variant="default" onClick={() => onDialog('goals')}>
              目標詳細設定…
            </Button>
            <Button
              size="xs"
              variant="default"
              disabled={!bandsUsable}
              onClick={() => onDialog('bands')}
              rightSection={setup.bands.length >= 2 && bandsUsable ? <Badge size="xs">{setup.bands.length}</Badge> : undefined}
            >
              バンド設定…
            </Button>
          </Group>
        </Stack>
      </div>

      {info && info.errors.length > 0 && (
        <Alert mt="sm" color="red" icon={<IconAlertTriangle size={16} />} title="このままでは実行できません">
          {info.errors.map((e, k) => (
            <div key={k}>{issueMessage(e)}</div>
          ))}
        </Alert>
      )}
      {info && info.warnings.length > 0 && (
        <Alert mt="sm" color="yellow" icon={<IconAlertTriangle size={16} />}>
          {info.warnings.map((e, k) => (
            <div key={k}>{issueMessage(e)}</div>
          ))}
        </Alert>
      )}

      <Group justify="space-between" mt="sm" wrap="nowrap">
        <Text size="xs" c="dimmed">
          各変数を 1 番から順に、評価値が上がる方向へ動かします。すべての変数で改善がなくなると終了し、結果をアンテナに反映します。
        </Text>
        <Group gap="xs" wrap="nowrap">
          {outcome && (
            <Button variant="default" leftSection={<IconListDetails size={14} />} onClick={() => setOptimizer({ resultsOpen: true })}>
              最適化結果の一覧
            </Button>
          )}
          <Button variant="default" onClick={onClose}>
            閉じる
          </Button>
          <Button leftSection={<IconPlayerPlay size={14} />} disabled={!info || info.errors.length > 0} onClick={onRun}>
            実行
          </Button>
        </Group>
      </Group>
    </>
  );
}

// ------------------------------------------------------------------ run ---

const STOP_TEXT: Record<string, string> = {
  converged: 'すべての変数で改善がなくなったので終了しました',
  cancelled: '中止しました（それまでの最良の状態を反映しました）',
  calculationLimit: '計算回数の上限に達しました（それまでの最良の状態を反映しました）',
};

function elapsed(ms: number) {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

function RunView({ project, elements, onBack, onClose }: { project: Project; elements: ElementRow[]; onBack: () => void; onClose: () => void }) {
  const opt = useApp((s) => s.optimizer);
  const setOptimizer = useApp((s) => s.setOptimizer);
  const cancel = useApp((s) => s.cancelOptimization);
  const [now, setNow] = useState(performance.now());
  useEffect(() => {
    if (!opt.running) return;
    const t = setInterval(() => setNow(performance.now()), 500);
    return () => clearInterval(t);
  }, [opt.running]);
  const logRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = logRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [opt.log]);

  const setup = opt.outcomeSetup && !opt.running ? opt.outcomeSetup : opt.setup;
  const multiband = setup.bands.length >= 2 && !setup.sweepOnly;
  const showCurrent = !setup.sweepOnly && setup.weights.current > 0;
  const last = opt.log.at(-1);
  const calculations = opt.running ? (last?.calculation ?? 0) : (opt.outcome?.calculations ?? last?.calculation ?? 0);
  const time = opt.startedAt != null ? (opt.finishedAt ?? now) - opt.startedAt : 0;
  const best = opt.running ? opt.best : (opt.outcome?.best ?? null);
  const stop = opt.outcome?.stop;

  return (
    <Stack gap="sm">
      <Group gap="lg">
        {opt.running ? (
          <Group gap="xs">
            <Loader size="xs" />
            <Text size="sm" fw={600}>
              最適化中…
            </Text>
          </Group>
        ) : opt.error ? (
          <Text size="sm" c="red" fw={600}>
            最適化を実行できませんでした
          </Text>
        ) : stop ? (
          <Text size="sm" fw={600} c={stop.kind === 'failed' ? 'red' : undefined}>
            {stop.kind === 'failed' ? `計算できない状態になったので終了しました（${stop.message}）` : STOP_TEXT[stop.kind]}
          </Text>
        ) : null}
        <Text size="sm" c="dimmed" className="num">
          計算 {calculations} 回・経過 {elapsed(time)}
        </Text>
        <Text size="xs" c="dimmed">
          {tradeOffText(setup)}
        </Text>
      </Group>
      {opt.error && (
        <Alert color="red" icon={<IconAlertTriangle size={16} />} title={opt.error.title}>
          {opt.error.detail}
        </Alert>
      )}

      <div style={{ display: 'flex', gap: 12, height: '52vh', minHeight: 300 }}>
        <div ref={logRef} style={{ flex: 1, minWidth: 0, overflow: 'auto', border: '1px solid var(--panel-border)', borderRadius: 8 }}>
          <Table striped fz="xs" className="num" stickyHeader horizontalSpacing={6} verticalSpacing={2}>
            <Table.Thead>
              <Table.Tr>
                <Table.Th ta="right">No.</Table.Th>
                <Table.Th ta="right">変数</Table.Th>
                <Table.Th ta="right">値</Table.Th>
                {multiband && <Table.Th ta="right">MHz</Table.Th>}
                <Table.Th ta="right">R</Table.Th>
                <Table.Th ta="right">jX</Table.Th>
                <Table.Th ta="right">SWR</Table.Th>
                <Table.Th ta="right">Ga</Table.Th>
                <Table.Th ta="right">F/B</Table.Th>
                <Table.Th ta="right">{showCurrent ? '電流 (mA)' : '仰角'}</Table.Th>
                <Table.Th />
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {opt.log.map((p) => (
                <LogRow key={`${p.calculation}-${p.band}`} p={p} multiband={multiband} showCurrent={showCurrent} />
              ))}
            </Table.Tbody>
          </Table>
          {opt.log.length === 0 && (opt.running || transport.kind === 'http') && (
            <Text size="sm" c="dimmed" ta="center" mt="xl">
              {transport.kind === 'http'
                ? 'ブラウザ版では途中経過を表示できません。試した結果は「最適化結果の一覧」で見られます。'
                : '最初の計算をしています…'}
            </Text>
          )}
        </div>
        <Stack gap={6} style={{ width: 300, flexShrink: 0, border: '1px solid var(--panel-border)', borderRadius: 8, padding: 12, overflow: 'auto' }}>
          <Text size="sm" fw={600}>
            {opt.running ? 'これまでの最良' : '最良の結果'}
          </Text>
          {best ? (
            <>
              <Text size="xs" className="num">
                Z = {best.impedance.re.toFixed(2)} {best.impedance.im < 0 ? '−' : '+'} j{Math.abs(best.impedance.im).toFixed(2)} Ω・SWR {best.swr.toFixed(2)}
              </Text>
              <Text size="xs" className="num">
                Ga {best.gainDbi.toFixed(2)} dBi・F/B {best.fbDb.toFixed(2)} dB・仰角 {best.elevationDeg.toFixed(1)}°
              </Text>
              <Table fz="xs" className="num" verticalSpacing={1} horizontalSpacing={4} mt={4}>
                <Table.Tbody>
                  {best.values.map((v, i) => (
                    <Table.Tr key={i}>
                      <Table.Td c="dimmed">{i + 1}</Table.Td>
                      <Table.Td>{setup.variables[i] ? describeTarget(setup.variables[i].target, project, elements) : ''}</Table.Td>
                      <Table.Td ta="right">{strDbl(v)}</Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            </>
          ) : (
            <Text size="xs" c="dimmed">
              まだありません
            </Text>
          )}
        </Stack>
      </div>

      <Group justify="flex-end" gap="xs">
        {opt.running ? (
          transport.canCancelOptimize && (
            <Button color="red" variant="light" leftSection={<IconPlayerStop size={14} />} onClick={() => void cancel()}>
              中止
            </Button>
          )
        ) : (
          <>
            <Button variant="default" onClick={onBack}>
              設定に戻る
            </Button>
            {opt.outcome && opt.outcome.trials.length > 0 && (
              <Button variant="default" leftSection={<IconListDetails size={14} />} onClick={() => setOptimizer({ resultsOpen: true })}>
                最適化結果の一覧
              </Button>
            )}
            <Button onClick={onClose}>閉じる</Button>
          </>
        )}
      </Group>
    </Stack>
  );
}

function LogRow({ p, multiband, showCurrent }: { p: Progress; multiband: boolean; showCurrent: boolean }) {
  return (
    <Table.Tr style={p.improved ? { fontWeight: 650 } : undefined}>
      <Table.Td ta="right">{p.calculation}</Table.Td>
      <Table.Td ta="right">{p.variable + 1}</Table.Td>
      <Table.Td ta="right">{strDbl(p.values[p.variable] ?? 0)}</Table.Td>
      {multiband && <Table.Td ta="right">{strDbl(p.frequencyMhz)}</Table.Td>}
      <Table.Td ta="right">{p.impedance.re.toFixed(1)}</Table.Td>
      <Table.Td ta="right">{p.impedance.im.toFixed(1)}</Table.Td>
      <Table.Td ta="right">{p.swr >= 99999 ? '∞' : p.swr.toFixed(2)}</Table.Td>
      <Table.Td ta="right">{p.gainDbi.toFixed(2)}</Table.Td>
      <Table.Td ta="right">{p.fbDb.toFixed(2)}</Table.Td>
      <Table.Td ta="right">{showCurrent && p.current != null ? (p.current * 1000).toFixed(1) : p.elevationDeg.toFixed(1)}</Table.Td>
      <Table.Td c="var(--mantine-primary-color-filled)">{p.improved ? '★' : ''}</Table.Td>
    </Table.Tr>
  );
}

// -------------------------------------------------------- goal details ---

/** 目標詳細設定: the goals' limits, the matching network and the current goal; also the F/B range and SWR reference of the environment. */
function GoalsDialog({ goals, project, onClose, onApply }: { goals: Goals; project: Project; onClose: () => void; onApply: (g: Goals) => void }) {
  const env = project.environment;
  const [g, setG] = useState(goals);
  const [fb, setFb] = useState(env.fbRange);
  const [ref, setRef] = useState(env.swrReference);
  const impedance = g.matching.kind === 'impedance' ? g.matching : null;
  const [r, setR] = useState(impedance?.r != null ? strDbl(impedance.r) : strDbl(env.swrReference.r));
  const [x, setX] = useState(impedance ? (impedance.x != null ? strDbl(impedance.x) : '*') : '0');
  const readZ = (t: string) => (t.trim() === '*' ? null : Number(t));
  const zValid = (t: string) => t.trim() === '*' || (t.trim() !== '' && Number.isFinite(Number(t)));

  const apply = () => {
    const matching: Goals['matching'] = g.matching.kind === 'impedance' ? { kind: 'impedance', r: readZ(r), x: readZ(x) } : g.matching;
    onApply({ ...g, matching });
    if (fb.horizontal !== env.fbRange.horizontal || fb.vertical !== env.fbRange.vertical || ref.r !== env.swrReference.r || ref.x !== env.swrReference.x) {
      useApp.getState().edit((p) => {
        p.environment.fbRange = fb;
        p.environment.swrReference = ref;
      });
    }
    onClose();
  };
  const autoJx = () => {
    let target = ref.r < 50 ? 50 : ref.r * 2;
    modals.openConfirmModal({
      title: '整合目標の Z',
      children: <NumberInput label="整合目標の Z (Ω)" defaultValue={target} min={0} onChange={(v) => (target = Number(v))} data-autofocus />,
      labels: { confirm: 'jX を求める', cancel: 'キャンセル' },
      onConfirm: () => {
        const jx = matchingJx(ref.r, target);
        if (jx != null) setRef({ ...ref, x: Number(jx.toFixed(6)) });
      },
    });
  };
  const num = (v: string | number) => (typeof v === 'number' ? v : Number(v));

  return (
    <Modal opened onClose={onClose} title="目標詳細設定" size="lg">
      <Stack gap="sm">
        <Text size="sm" fw={600}>
          許容最大目標
        </Text>
        <Text size="xs" c="dimmed">
          これを超えると評価がほとんど増えず（jX・SWR は下回ると減らず）、ほかの目標が重視されます。
        </Text>
        <Group grow>
          <NumberInput size="xs" label="Gain (dBi)" value={g.maxGainDbi} min={0.001} onChange={(v) => num(v) > 0 && setG({ ...g, maxGainDbi: num(v) })} />
          <NumberInput size="xs" label="F/B (dB)" value={g.maxFbDb} min={0.001} onChange={(v) => num(v) > 0 && setG({ ...g, maxFbDb: num(v) })} />
          <NumberInput size="xs" label="jX (Ω)" value={g.minJx} min={0} onChange={(v) => num(v) >= 0 && setG({ ...g, minJx: num(v) })} />
          <NumberInput size="xs" label="SWR" value={g.minSwr} min={1} step={0.1} decimalScale={3} onChange={(v) => num(v) >= 1 && setG({ ...g, minSwr: num(v) })} />
        </Group>

        <Text size="sm" fw={600} mt="xs">
          マッチング回路
        </Text>
        <SegmentedControl
          size="xs"
          value={g.matching.kind}
          onChange={(k) =>
            setG({ ...g, matching: k === 'impedance' ? { kind: 'impedance', r: readZ(r), x: readZ(x) } : { kind: k as 'capacitance' | 'hairpin' } })
          }
          data={[
            { value: 'capacitance', label: 'キャパシタンスマッチ' },
            { value: 'hairpin', label: 'ヘアピンマッチ' },
            { value: 'impedance', label: '任意の Z' },
          ]}
        />
        {g.matching.kind === 'impedance' && (
          <Group grow>
            <TextInput size="xs" label="R (Ω)" description="* で評価しない" value={r} onChange={(e) => setR(e.currentTarget.value)} error={!zValid(r)} />
            <TextInput size="xs" label="jX (Ω)" description="* で評価しない" value={x} onChange={(e) => setX(e.currentTarget.value)} error={!zValid(x)} />
          </Group>
        )}

        <Text size="sm" fw={600} mt="xs">
          電流目標
        </Text>
        <Group align="end">
          <TextInput size="xs" label="パルス位置" w={140} value={g.currentPulse} onChange={(e) => setG({ ...g, currentPulse: e.currentTarget.value })} />
          <Radio.Group value={g.minimizeCurrent ? 'min' : 'max'} onChange={(v) => setG({ ...g, minimizeCurrent: v === 'min' })}>
            <Group gap="md" mb={4}>
              <Radio size="xs" value="max" label="最大値" />
              <Radio size="xs" value="min" label="最小値" />
            </Group>
          </Radio.Group>
        </Group>

        <Text size="sm" fw={600} mt="xs">
          計算環境
        </Text>
        <Group grow align="end">
          <NumberInput size="xs" label="F/B 比の後方範囲 水平 (°)" value={fb.horizontal} min={0} max={180} allowDecimal={false} onChange={(v) => setFb({ ...fb, horizontal: num(v) })} />
          <NumberInput size="xs" label="垂直 (°)" value={fb.vertical} min={0} max={90} allowDecimal={false} onChange={(v) => setFb({ ...fb, vertical: num(v) })} />
        </Group>
        <Group grow align="end">
          <NumberInput size="xs" label="SWR 計算基準 R (Ω)" value={ref.r} min={0.001} onChange={(v) => num(v) > 0 && setRef({ ...ref, r: num(v) })} />
          <NumberInput size="xs" label="jX (Ω)" value={ref.x} onChange={(v) => Number.isFinite(num(v)) && setRef({ ...ref, x: num(v) })} />
          <Button size="xs" variant="default" onClick={autoJx}>
            整合用の jX を求める…
          </Button>
        </Group>

        <Group justify="flex-end" gap="xs" mt="sm">
          <Button variant="default" onClick={onClose}>
            キャンセル
          </Button>
          <Button disabled={g.matching.kind === 'impedance' && (!zValid(r) || !zValid(x))} onClick={apply}>
            OK
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

// ---------------------------------------------------------------- bands ---

const BAND_SOURCES = 4;

/** 試行バンド設定: frequencies scored together, each with its sources. */
function BandsDialog({
  setup,
  project,
  onClose,
  onApply,
}: {
  setup: OptimizerSetup;
  project: Project;
  onClose: () => void;
  onApply: (p: Pick<OptimizerSetup, 'bands' | 'emphasizeFirstBand'>) => void;
}) {
  const options = useApp((s) => s.options);
  const [bands, setBands] = useState<Band[]>(() =>
    setup.bands.length >= 2
      ? setup.bands
      : [{ frequencyMhz: options.frequencyMhz ?? project.antenna.frequencyMhz, sources: project.antenna.sources.slice(0, BAND_SOURCES) }],
  );
  const [emphasize, setEmphasize] = useState(setup.emphasizeFirstBand);
  const done = () => {
    onApply({ bands: bands.length >= 2 ? bands : [], emphasizeFirstBand: emphasize });
    onClose();
  };
  const patch = (i: number, b: Band) => setBands(bands.map((x, k) => (k === i ? b : x)));
  const setSource = (i: number, n: number, s: Partial<Band['sources'][number]>) => {
    const sources = [...bands[i].sources];
    while (sources.length <= n) sources.push({ pulse: '', phaseDeg: 0, voltage: 1 });
    sources[n] = { ...sources[n], ...s };
    patch(i, { ...bands[i], sources: sources.filter((x, k) => x.pulse.trim() !== '' || k < n) });
  };

  return (
    <Modal opened onClose={done} title="試行バンド設定" size="min(1100px, 94vw)">
      <Text size="xs" c="dimmed" mb="xs">
        2 つ以上のバンドを設定すると、変数を 1 回変えるたびにすべてのバンドで計算し、評価値を合算します（試行回数はバンドの数だけ増えます）。最適化結果の一覧に出るのは 1 番のバンドの結果です。
      </Text>
      <ScrollArea type="auto" style={{ border: '1px solid var(--panel-border)', borderRadius: 8 }}>
        <table className="grid-table" aria-label="試行バンド">
          <thead>
            <tr>
              <th style={{ textAlign: 'center' }}>No.</th>
              <th>周波数 (MHz)</th>
              {Array.from({ length: BAND_SOURCES }, (_, n) => (
                <SourceHeads key={n} n={n} />
              ))}
              <th />
            </tr>
          </thead>
          <tbody>
            {bands.map((b, i) => (
              <tr key={i}>
                <td className="index num">{i + 1}</td>
                <td style={{ minWidth: 80 }}>
                  <NumCell ariaLabel={`バンド ${i + 1} の周波数`} value={b.frequencyMhz} digits={6} validate={(v) => v > 1.5 && v < 999999.9} onCommit={(v) => patch(i, { ...b, frequencyMhz: v })} />
                </td>
                {Array.from({ length: BAND_SOURCES }, (_, n) => {
                  const s = b.sources[n];
                  return (
                    <SourceCells
                      key={n}
                      label={`バンド ${i + 1} の給電点 ${n + 1}`}
                      pulse={s?.pulse ?? ''}
                      phase={s?.phaseDeg}
                      voltage={s?.voltage}
                      onPulse={(pulse) => setSource(i, n, { pulse })}
                      onPhase={(phaseDeg) => setSource(i, n, { phaseDeg })}
                      onVoltage={(voltage) => setSource(i, n, { voltage })}
                    />
                  );
                })}
                <td style={{ width: 30 }}>
                  <ActionIcon size="sm" color="red" aria-label={`バンド ${i + 1} を削除`} disabled={bands.length <= 1} onClick={() => setBands(bands.filter((_, k) => k !== i))}>
                    <IconTrash size={14} />
                  </ActionIcon>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </ScrollArea>
      <Group justify="space-between" mt="sm">
        <Group gap="md">
          <Button
            size="xs"
            variant="default"
            leftSection={<IconPlus size={14} />}
            disabled={bands.length >= 10}
            onClick={() => setBands([...bands, structuredClone(bands[bands.length - 1])])}
          >
            バンドを追加
          </Button>
          <Tooltip label="1 番のバンドはそのまま、ほかのバンドは評価値をバンド数で割って合算します" multiline w={260}>
            <Checkbox size="xs" label="1 番を重視" checked={emphasize} onChange={(e) => setEmphasize(e.currentTarget.checked)} />
          </Tooltip>
        </Group>
        <Button onClick={done}>閉じる</Button>
      </Group>
    </Modal>
  );
}

function SourceHeads({ n }: { n: number }) {
  return (
    <>
      <th className="left">パルス{n + 1}</th>
      <th>位相{n + 1}</th>
      <th>電圧{n + 1}</th>
    </>
  );
}

function SourceCells(p: {
  label: string;
  pulse: string;
  phase?: number;
  voltage?: number;
  onPulse: (v: string) => void;
  onPhase: (v: number) => void;
  onVoltage: (v: number) => void;
}) {
  const has = p.pulse.trim() !== '';
  return (
    <>
      <td style={{ minWidth: 56 }}>
        <TextCell ariaLabel={`${p.label} のパルス`} value={p.pulse} onCommit={p.onPulse} />
      </td>
      <td style={{ minWidth: 48 }}>
        {has && <NumCell ariaLabel={`${p.label} の位相`} value={p.phase ?? 0} digits={3} validate={(v) => v >= 0 && v < 360} onCommit={p.onPhase} />}
      </td>
      <td style={{ minWidth: 48 }}>
        {has && <NumCell ariaLabel={`${p.label} の電圧`} value={p.voltage ?? 1} digits={4} validate={(v) => v > 0} onCommit={p.onVoltage} />}
      </td>
    </>
  );
}

// ---------------------------------------------------------- all elements ---

const AXIS_LABELS: Record<ElementAxis, string> = { width: '横幅', height: '縦幅', xWidth: 'Ｘ幅' };

/** 全エレメント, with the question MMANA asks for elements with two or more of width, height and X width. */
function AllElementsDialog({ ask, replaces, onClose, onApply }: { ask: boolean; replaces: boolean; onClose: () => void; onApply: (op: SetupOp) => void }) {
  const [flags, setFlags] = useState(elementEditorFlags);
  const [c, setC] = useState<ElementCombination>({ width: true, height: true, xWidth: true, widthLink: null, heightLink: null, xWidthLink: null });
  const axes: { key: 'width' | 'height' | 'xWidth'; link: 'widthLink' | 'heightLink' | 'xWidthLink'; others: ElementAxis[] }[] = [
    { key: 'width', link: 'widthLink', others: ['height', 'xWidth'] },
    { key: 'height', link: 'heightLink', others: ['width', 'xWidth'] },
    { key: 'xWidth', link: 'xWidthLink', others: ['width', 'height'] },
  ];
  return (
    <Modal opened onClose={onClose} title="全エレメント" size="md">
      <Stack gap="sm">
        <Text size="sm">
          すべてのエレメントの幅（ループは全周）と間隔または位置を、ラジエータ・リフレクタ・ディレクタの順に変数として登録します。同じ X 位置で値の等しいものは連携させます。
        </Text>
        <SegmentedControl
          size="xs"
          value={flags.intervals ? 'interval' : 'position'}
          onChange={(v) => setFlags({ ...flags, intervals: v === 'interval' })}
          data={[
            { value: 'interval', label: '間隔を変数にする' },
            { value: 'position', label: '位置を変数にする' },
          ]}
        />
        <SegmentedControl
          size="xs"
          value={flags.scaleAll ? 'all' : 'edge'}
          onChange={(v) => setFlags({ ...flags, scaleAll: v === 'all' })}
          data={[
            { value: 'edge', label: '端点のみ変更' },
            { value: 'all', label: '全ての座標を同一比率で変更' },
          ]}
        />
        {ask && (
          <>
            <Text size="sm" fw={600} mt="xs">
              このアンテナには横幅・縦幅・Ｘ幅のうち 2 つ以上を持つエレメントがあります
            </Text>
            {axes.map(({ key, link, others }) => (
              <Group key={key} gap="md" wrap="nowrap">
                <Checkbox size="xs" w={110} label={`${AXIS_LABELS[key]}を登録`} checked={c[key]} onChange={(e) => setC({ ...c, [key]: e.currentTarget.checked })} />
                <SegmentedControl
                  size="xs"
                  disabled={!c[key]}
                  value={c[link] ?? 'off'}
                  onChange={(v) => setC({ ...c, [link]: v === 'off' ? null : (v as ElementAxis) })}
                  data={[{ value: 'off', label: '連携なし' }, ...others.map((o) => ({ value: o, label: `${AXIS_LABELS[o]}に連携`, disabled: !c[o] }))]}
                />
              </Group>
            ))}
          </>
        )}
        {replaces && (
          <Text size="xs" c="orange">
            いまの変数はすべて置き換わります。
          </Text>
        )}
        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={onClose}>
            キャンセル
          </Button>
          <Button
            onClick={() => {
              onApply({ op: 'allElements', intervals: flags.intervals, scaleAll: flags.scaleAll, combination: ask ? c : null });
              onClose();
            }}
          >
            登録
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

function PitchDialog({ initial, percent, onClose, onApply }: { initial: number; percent: boolean; onClose: () => void; onApply: (p: number) => void }) {
  const [v, setV] = useState<number | string>(initial);
  const n = typeof v === 'number' ? v : Number(v);
  return (
    <Modal opened onClose={onClose} title="ピッチの一括設定" size="xs">
      <Stack gap="sm">
        <NumberInput label={percent ? 'ピッチ (%)' : 'ピッチ'} description="すべての独立変数に設定します" value={v} onChange={setV} decimalScale={6} data-autofocus />
        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={onClose}>
            キャンセル
          </Button>
          <Button
            disabled={!Number.isFinite(n)}
            onClick={() => {
              onApply(n);
              onClose();
            }}
          >
            設定
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}
