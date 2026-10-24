import { ActionIcon, Button, Checkbox, Group, Popover, Radio, SegmentedControl, Stack, Text, TextInput, Tooltip } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import {
  IconChevronDown,
  IconChevronRight,
  IconCopy,
  IconFolderMinus,
  IconFolderPlus,
  IconLayoutRows,
  IconPencil,
  IconPlus,
  IconSettings,
  IconTrash,
} from '@tabler/icons-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { Point3 } from '../api/types';
import {
  lengthDigits,
  lengthFromDisplay,
  lengthToDisplay,
  lengthUnit,
  lengthWavelength,
  radiusDigits,
  radiusFromDisplay,
  radiusToDisplay,
  radiusUnit,
  radiusWavelength,
  type Units,
} from '../domain/elements';
import { byWire, worst, type Diagnostic } from '../domain/diagnostics';
import { evaluate } from '../domain/expr';
import { wavelength, wireLength } from '../domain/project';
import { describeSegmentation, formatSegmentation, parseSegmentation } from '../domain/segmentation';
import { addedWire, insertWires, removeWires, type NewWireMode } from '../domain/wireEdit';
import { listRows, type ListRow } from '../domain/wireGroups';
import { formatKey } from '../domain/shortcuts';
import { useDiagnostics } from '../state/diagnostics';
import { useDisplay } from '../state/display';
import { useEditor } from '../state/editor';
import { useInput } from '../state/input';
import { useNewWire } from '../state/newWire';
import { useApp } from '../state/store';
import {
  canEditWires,
  dissolveGroup,
  duplicateWireGroup,
  groupSelected,
  moveGroup,
  renameWireGroup,
  reorderWires,
  selectGroup,
  ungroupSelected,
} from '../state/wireCommands';
import { NumCell, TextCell, format } from './Cells';
import { useWireDrag, type DragView } from './useWireDrag';

const COORDS: { end: 'start' | 'end'; axis: keyof Point3; label: string }[] = [
  { end: 'start', axis: 'x', label: 'X1' },
  { end: 'start', axis: 'y', label: 'Y1' },
  { end: 'start', axis: 'z', label: 'Z1' },
  { end: 'end', axis: 'x', label: 'X2' },
  { end: 'end', axis: 'y', label: 'Y2' },
  { end: 'end', axis: 'z', label: 'Z2' },
];

/** No., the coordinates, R, SEG, 長さ, メモ and the row buttons. */
const COLUMNS = COORDS.length + 6;

export function WireTable() {
  const wires = useApp((s) => s.project.antenna.wires);
  const freq = useApp((s) => s.project.antenna.frequencyMhz);
  const selected = useApp((s) => s.selectedWire);
  const selection = useApp((s) => s.selectedWires);
  const select = useApp((s) => s.selectWire);
  const setSelection = useApp((s) => s.setSelection);
  const toggleWire = useApp((s) => s.toggleWire);
  const edit = useApp((s) => s.edit);
  const setElementEditorOpen = useApp((s) => s.setElementEditorOpen);
  const lambda = wavelength(freq);
  const display = useDisplay();
  // 波長表記 needs a design frequency
  const units: Units = { wavelengths: display.wavelengths && lambda > 0, millimetres: display.millimetres, lambda };
  const lu = lengthUnit(units);
  const bodyRef = useRef<HTMLTableSectionElement>(null);
  const diagnostics = useDiagnostics();
  const problems = useMemo(() => byWire(diagnostics), [diagnostics]);
  const collapsedGroups = useEditor((s) => s.collapsedGroups);
  const renaming = useEditor((s) => s.renamingGroup);
  const setEditor = useEditor((s) => s.set);
  const isCollapsed = (name: string) => collapsedGroups.includes(name);
  // the wires of a folded group are not shown, only its heading
  const rows = useMemo(
    () => listRows(wires).filter((r) => r.kind === 'group' || !(r.group && collapsedGroups.includes(r.group))),
    [wires, collapsedGroups],
  );
  const drag = useWireDrag(bodyRef, rows, wires.length, isCollapsed, (s, drop) =>
    s.dragged.kind === 'group' ? moveGroup(s.dragged.name, s.wires, drop.at) : reorderWires(s.wires, drop.at, drop.group),
  );
  const bindings = useInput((s) => s.bindings);
  const keyHint = (id: 'group' | 'ungroup') => (bindings[id][0] ? `（${formatKey(bindings[id][0])}）` : '');
  const grouped = selection.some((i) => wires[i]?.group);

  // keep the row selected in the 3D view visible (the heading, when its group is folded)
  useEffect(() => {
    if (selected == null) return;
    const row = bodyRef.current?.querySelector(`[data-wire="${selected}"]`);
    const group = wires[selected]?.group;
    const head = group ? [...(bodyRef.current?.querySelectorAll<HTMLElement>('[data-group]') ?? [])].find((h) => h.dataset.group === group) : null;
    (row ?? head)?.scrollIntoView({ block: 'nearest' });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected]);

  const toggleCollapsed = (name: string) =>
    setEditor({ collapsedGroups: isCollapsed(name) ? collapsedGroups.filter((n) => n !== name) : [...collapsedGroups, name] });

  const finishRename = (from: string, to: string | null) => {
    setEditor({ renamingGroup: null });
    const name = to?.trim();
    if (!name || name === from) return;
    renameWireGroup(from, name);
    if (isCollapsed(from)) setEditor({ collapsedGroups: [...collapsedGroups.filter((n) => n !== from), name] });
  };

  /** Drags the selection when the wire is in it, otherwise the wire alone. */
  const dragWire = (i: number, e: React.PointerEvent) => {
    if (!canEditWires()) return;
    const moving = selection.includes(i) ? [...selection].sort((a, b) => a - b) : [i];
    drag.start(e, {
      wires: moving,
      dragged: { kind: 'wires' },
      label: moving.length === 1 ? `ワイヤー ${moving[0] + 1}` : `ワイヤー ${moving.length} 本`,
      begin: () => !selection.includes(i) && select(i),
    });
  };

  const dragGroup = (r: Extract<ListRow, { kind: 'group' }>, e: React.PointerEvent) => {
    if (!canEditWires() || renaming === r.name) return;
    drag.start(e, { wires: r.wires, dragged: { kind: 'group', name: r.name, wires: r.wires }, label: `グループ「${r.name}」（${r.wires.length} 本）` });
  };

  const addWire = () => {
    const settings = useNewWire.getState();
    edit((p) => void p.antenna.wires.push(addedWire(p.antenna.wires.at(-1), settings)));
    select(wires.length);
  };

  /** Click on a row number: Shift selects a range, ⌘ / Ctrl adds or removes. */
  const clickRow = (i: number, e: React.MouseEvent) => {
    if (drag.wasDrag()) return;
    if (e.shiftKey && selected != null) {
      const [a, b] = selected < i ? [selected, i] : [i, selected];
      setSelection([...selection, ...Array.from({ length: b - a + 1 }, (_, k) => a + k)], i);
    } else if (e.metaKey || e.ctrlKey) {
      toggleWire(i);
    } else {
      select(selected === i && selection.length === 1 ? null : i);
    }
  };

  /** Replaces the antenna and tells the user about sources and loads that moved or went. */
  const editPulses = (r: ReturnType<typeof removeWires>) => {
    edit((p) => void (p.antenna = r.antenna));
    if (r.warnings.length) notifications.show({ color: 'yellow', title: '給電点・負荷を更新しました', message: r.warnings.join('\n') });
  };

  return (
    <>
      <table className="grid-table" aria-label="ワイヤー">
        <thead>
          <tr>
            <th style={{ textAlign: 'center' }}>No.</th>
            {COORDS.map((c) => (
              <th key={c.label}>
                {c.label} ({lu})
              </th>
            ))}
            <th>R ({radiusUnit(units)})</th>
            <th>
              <Tooltip label="分割数。正: 等分、0: 自動等分、-1: 両端テーパ、-2/-3: 始点/終点テーパ、「400,40」: λ/400〜λ/40 のテーパ" multiline w={260}>
                <span>SEG</span>
              </Tooltip>
            </th>
            <th>長さ ({lu})</th>
            <th className="left">メモ</th>
            <th />
          </tr>
        </thead>
        <tbody ref={bodyRef}>
          {rows.map((r, k) => {
            if (r.kind === 'group') {
              const first = rows.findIndex((o) => o.kind === 'group' && o.name === r.name) === k;
              return (
                <GroupHeading
                  key={`g${r.wires[0]}`}
                  row={k}
                  name={r.name}
                  count={r.wires.length}
                  selected={r.wires.filter((i) => selection.includes(i)).length / r.wires.length}
                  collapsed={isCollapsed(r.name)}
                  renaming={renaming === r.name && first}
                  onPointerDown={(e) => dragGroup(r, e)}
                  onClick={(e) => !drag.wasDrag() && selectGroup(r.name, e.shiftKey || e.metaKey || e.ctrlKey)}
                  onToggle={() => toggleCollapsed(r.name)}
                  onRename={() => canEditWires() && setEditor({ renamingGroup: r.name })}
                  onRenamed={(to) => finishRename(r.name, to)}
                  onDuplicate={() => duplicateWireGroup(r.name, r.wires)}
                  onDissolve={() => dissolveGroup(r.name)}
                />
              );
            }
            const i = r.wire;
            const w = wires[i];
            return (
              <tr
                key={`w${i}`}
                data-row={k}
                data-wire={i}
                className={[selection.includes(i) && 'selected', r.group && 'in-group'].filter(Boolean).join(' ') || undefined}
                onFocus={() => (selection.includes(i) ? setSelection(selection, i) : select(i))}
              >
                <WireNumber index={i} problems={problems.get(i) ?? []} onClick={(e) => clickRow(i, e)} onPointerDown={(e) => dragWire(i, e)} />
                {COORDS.map((c) => (
                  <td key={c.label} style={{ minWidth: 50 }}>
                    <NumCell
                      ariaLabel={`ワイヤー ${i + 1} ${c.label}`}
                      value={lengthToDisplay(w[c.end][c.axis], units)}
                      digits={lengthDigits(units)}
                      wavelength={lengthWavelength(units)}
                      onCommit={(v) => edit((p) => void (p.antenna.wires[i][c.end][c.axis] = lengthFromDisplay(v, units)))}
                    />
                  </td>
                ))}
                <td style={{ minWidth: 50 }}>
                  <NumCell
                    ariaLabel={`ワイヤー ${i + 1} 半径`}
                    value={radiusToDisplay(w.radius, units)}
                    digits={radiusDigits(units)}
                    wavelength={radiusWavelength(units)}
                    onCommit={(v) => edit((p) => void (p.antenna.wires[i].radius = radiusFromDisplay(v, units)))}
                  />
                </td>
                <td style={{ minWidth: 46 }}>
                  <Tooltip label={describeSegmentation(w.segmentation)} openDelay={600}>
                    <div>
                      <TextCell
                        ariaLabel={`ワイヤー ${i + 1} 分割`}
                        align="right"
                        value={formatSegmentation(w.segmentation)}
                        validate={(t) => parseSegmentation(t) != null}
                        onCommit={(t) => edit((p) => void (p.antenna.wires[i].segmentation = parseSegmentation(t)!))}
                      />
                    </div>
                  </Tooltip>
                </td>
                <td className="readonly num">{format(lengthToDisplay(wireLength(w), units), lengthDigits(units))}</td>
                <td style={{ minWidth: 60 }}>
                  <TextCell
                    ariaLabel={`ワイヤー ${i + 1} メモ`}
                    value={w.note ?? ''}
                    onCommit={(t) => edit((p) => void (p.antenna.wires[i].note = t))}
                  />
                </td>
                <td style={{ width: 52 }}>
                  <Group gap={0} wrap="nowrap" className="row-actions">
                    <Tooltip label="複製">
                      <ActionIcon
                        size="sm"
                        aria-label={`ワイヤー ${i + 1} を複製`}
                        onClick={() => editPulses(insertWires(useApp.getState().project.antenna, i + 1, [structuredClone(w)]))}
                      >
                        <IconCopy size={14} />
                      </ActionIcon>
                    </Tooltip>
                    <Tooltip label="削除">
                      <ActionIcon
                        size="sm"
                        color="red"
                        aria-label={`ワイヤー ${i + 1} を削除`}
                        onClick={() => {
                          editPulses(removeWires(useApp.getState().project.antenna, [i]));
                          select(null);
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
      {drag.view && <DragOverlay view={drag.view} />}
      <Group p="xs" justify="space-between">
        <Group gap="xs">
          <Group gap={0} wrap="nowrap">
            <Button variant="light" leftSection={<IconPlus size={14} />} onClick={addWire}>
              ワイヤーを追加
            </Button>
            <NewWireOptions units={units} />
          </Group>
          <Tooltip label="つながったワイヤをエレメントとして、幅・全周・間隔などをまとめて変更します" multiline w={260}>
            <Button variant="default" leftSection={<IconLayoutRows size={14} />} onClick={() => setElementEditorOpen(true)}>
              エレメント編集…
            </Button>
          </Tooltip>
          <Tooltip label={`選択したワイヤーを 1 つのグループにまとめ、最初のワイヤーの位置に並べます${keyHint('group')}`} multiline w={260}>
            <ActionIcon variant="default" size="input-sm" aria-label="グループにまとめる" disabled={selection.length === 0} onClick={() => groupSelected()}>
              <IconFolderPlus size={16} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label={`選択したワイヤーをグループから外します${keyHint('ungroup')}`}>
            <ActionIcon variant="default" size="input-sm" aria-label="グループを解除" disabled={!grouped} onClick={ungroupSelected}>
              <IconFolderMinus size={16} />
            </ActionIcon>
          </Tooltip>
        </Group>
        <Group gap="sm">
          <Text size="xs" c="dimmed">
            数値欄では R（波長 = {units.wavelengths ? '1 λ' : `${format(lengthWavelength(units), units.millimetres ? 1 : 3)} ${lu}`}）や四則演算が使えます
          </Text>
          <Tooltip label="座標と長さの単位。半径はいつも mm です">
            <SegmentedControl
              size="xs"
              aria-label="長さの単位"
              value={display.millimetres ? 'mm' : 'm'}
              disabled={units.wavelengths}
              onChange={(v) => display.setMillimetres(v === 'mm')}
              data={['m', 'mm']}
            />
          </Tooltip>
          <Tooltip label="座標・長さ・半径を設計周波数の波長で表します（エレメント編集と共通）" multiline w={240}>
            <div>
              <Checkbox
                size="xs"
                label="波長表記"
                checked={units.wavelengths}
                disabled={!(lambda > 0)}
                onChange={(e) => display.setWavelengths(e.currentTarget.checked)}
              />
            </div>
          </Tooltip>
        </Group>
      </Group>
    </>
  );
}

const NEW_WIRE_MODES: { mode: NewWireMode; label: string }[] = [
  { mode: 'zero', label: '原点に長さ 0（座標はあとで入力。MMANA と同じ）' },
  { mode: 'continue', label: '最後のワイヤーの終点につなげる（Y 方向に 1 m）' },
  { mode: 'offset', label: '最後のワイヤーをずらした位置' },
  { mode: 'fixed', label: '指定した座標' },
];

/** The settings of the 追加 button: where the new wire goes. */
function NewWireOptions({ units }: { units: Units }) {
  const s = useNewWire();
  const lu = lengthUnit(units);
  const point = (key: 'offset' | 'start' | 'end', labels: string[], disabled: boolean) => (
    <Group grow gap="xs" wrap="nowrap">
      {(['x', 'y', 'z'] as const).map((a, k) => (
        <LengthInput key={a} label={labels[k]} value={s[key][a]} units={units} disabled={disabled} onCommit={(v) => s.set({ [key]: { ...s[key], [a]: v } })} />
      ))}
    </Group>
  );
  return (
    <Popover position="top-start" shadow="md">
      <Popover.Target>
        <Tooltip label="追加するワイヤーの位置">
          <ActionIcon variant="light" size="input-sm" aria-label="追加するワイヤーの位置">
            <IconSettings size={16} />
          </ActionIcon>
        </Tooltip>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="xs" w={300}>
          <Radio.Group label="追加するワイヤーの位置" value={s.mode} onChange={(v) => s.set({ mode: v as NewWireMode })}>
            <Stack gap={6} mt={6}>
              {NEW_WIRE_MODES.map((m) => (
                <Radio key={m.mode} size="xs" value={m.mode} label={m.label} />
              ))}
            </Stack>
          </Radio.Group>
          <Text size="xs" fw={500}>
            ずらす量（{lu}）
          </Text>
          {point('offset', ['ΔX', 'ΔY', 'ΔZ'], s.mode !== 'offset')}
          <Text size="xs" fw={500}>
            指定した座標（{lu}）
          </Text>
          {point('start', ['X1', 'Y1', 'Z1'], s.mode !== 'fixed')}
          {point('end', ['X2', 'Y2', 'Z2'], s.mode !== 'fixed')}
          <Text size="xs" c="dimmed">
            半径と SEG は最後のワイヤーと同じにします。最後のワイヤーがないときは、ずらす・続けるの代わりに原点から Y 方向に 1 m のワイヤーを追加します。
          </Text>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

/** A length in the wire table's units; takes expressions like the cells (R = wavelength). */
function LengthInput({ label, value, units, disabled, onCommit }: { label: string; value: number; units: Units; disabled: boolean; onCommit: (m: number) => void }) {
  const shown = format(lengthToDisplay(value, units), lengthDigits(units));
  const [text, setText] = useState(shown);
  const [bad, setBad] = useState(false);
  useEffect(() => {
    setText(shown);
    setBad(false);
  }, [shown]);
  const commit = () => {
    if (text === shown) return setBad(false);
    const v = evaluate(text, { wavelength: lengthWavelength(units) });
    if (v == null) return setBad(true);
    setBad(false);
    onCommit(lengthFromDisplay(v, units));
    setText(format(v, lengthDigits(units)));
  };
  return (
    <TextInput
      size="xs"
      label={label}
      value={text}
      disabled={disabled}
      error={bad}
      classNames={{ input: 'num' }}
      onChange={(e) => setText(e.currentTarget.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
    />
  );
}

const SEVERITY_COLOR = { error: 'var(--diag-error)', warning: 'var(--diag-warning)', info: 'var(--diag-info)' };

/** The row number, coloured by the worst problem of the wire; the problems show on hover. Dragging it moves the wire. */
function WireNumber({
  index,
  problems,
  onClick,
  onPointerDown,
}: {
  index: number;
  problems: Diagnostic[];
  onClick: (e: React.MouseEvent) => void;
  onPointerDown: (e: React.PointerEvent) => void;
}) {
  const severity = worst(problems);
  return (
    <Tooltip
      disabled={severity == null}
      position="right"
      multiline
      w={320}
      openDelay={150}
      label={
        <Stack gap={2}>
          {problems.map((d, k) => (
            <Text key={k} size="xs" style={{ borderLeft: `3px solid ${SEVERITY_COLOR[d.severity]}`, paddingLeft: 6 }}>
              {d.message}
            </Text>
          ))}
        </Stack>
      }
    >
      <td
        className={`index num${severity ? ` diag-${severity}` : ''}`}
        aria-description={problems.map((d) => d.message).join('。') || undefined}
        onClick={onClick}
        onPointerDown={onPointerDown}
      >
        {index + 1}
      </td>
    </Tooltip>
  );
}

interface GroupHeadingProps {
  row: number;
  name: string;
  count: number;
  /** The part of its wires that is selected (0 to 1). */
  selected: number;
  collapsed: boolean;
  renaming: boolean;
  onPointerDown: (e: React.PointerEvent) => void;
  onClick: (e: React.MouseEvent) => void;
  onToggle: () => void;
  onRename: () => void;
  /** The new name, or null when renaming was cancelled. */
  onRenamed: (name: string | null) => void;
  onDuplicate: () => void;
  onDissolve: () => void;
}

/** The heading over a group's wires: click selects them, dragging moves them, double-click renames. */
function GroupHeading(p: GroupHeadingProps) {
  // the buttons and the name field do not start a drag or select the group
  const own = { onPointerDown: (e: React.PointerEvent) => e.stopPropagation(), onClick: (e: React.MouseEvent) => e.stopPropagation() };
  const state = p.selected === 1 ? 'selected' : p.selected > 0 ? 'partial' : '';
  return (
    <tr data-row={p.row} data-group={p.name} className={`group-row ${state}`} onPointerDown={p.onPointerDown} onClick={p.onClick}>
      <td colSpan={COLUMNS}>
        <div className="group-head">
          <ActionIcon size="sm" variant="subtle" color="gray" aria-label={p.collapsed ? `グループ「${p.name}」を開く` : `グループ「${p.name}」をたたむ`} {...own} onClick={(e) => (e.stopPropagation(), p.onToggle())}>
            {p.collapsed ? <IconChevronRight size={14} /> : <IconChevronDown size={14} />}
          </ActionIcon>
          {p.renaming ? (
            <GroupNameInput name={p.name} onDone={p.onRenamed} />
          ) : (
            <span className="group-name" onDoubleClick={(e) => (e.stopPropagation(), p.onRename())}>
              {p.name}
            </span>
          )}
          <Text span size="xs" c="dimmed">
            {p.count} 本
          </Text>
          <Group gap={0} wrap="nowrap" className="row-actions" ml="auto">
            <Tooltip label="名前を変更">
              <ActionIcon size="sm" aria-label={`グループ「${p.name}」の名前を変更`} {...own} onClick={(e) => (e.stopPropagation(), p.onRename())}>
                <IconPencil size={14} />
              </ActionIcon>
            </Tooltip>
            <Tooltip label="グループを複製（コピーしたワイヤーを新しいグループとして直後に追加）" multiline w={220}>
              <ActionIcon size="sm" aria-label={`グループ「${p.name}」を複製`} {...own} onClick={(e) => (e.stopPropagation(), p.onDuplicate())}>
                <IconCopy size={14} />
              </ActionIcon>
            </Tooltip>
            <Tooltip label="グループを解除（ワイヤーは残ります）">
              <ActionIcon size="sm" aria-label={`グループ「${p.name}」を解除`} {...own} onClick={(e) => (e.stopPropagation(), p.onDissolve())}>
                <IconFolderMinus size={14} />
              </ActionIcon>
            </Tooltip>
          </Group>
        </div>
      </td>
    </tr>
  );
}

/** The name field of a heading: Enter or leaving it renames, Escape cancels. */
function GroupNameInput({ name, onDone }: { name: string; onDone: (name: string | null) => void }) {
  // the field loses focus after Enter or Escape too; only the first counts
  const done = useRef(false);
  const finish = (to: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(to);
  };
  return (
    <TextInput
      size="xs"
      aria-label="グループ名"
      defaultValue={name}
      autoFocus
      onFocus={(e) => e.currentTarget.select()}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        // the list's and the app's keys do not apply while typing a name
        e.stopPropagation();
        if (e.key === 'Enter') finish(e.currentTarget.value);
        if (e.key === 'Escape') finish(null);
      }}
      onBlur={(e) => finish(e.currentTarget.value)}
    />
  );
}

/** Where a drop would put the wires, and what is being dragged, over everything else. */
function DragOverlay({ view }: { view: DragView }) {
  return createPortal(
    <>
      {view.line && <div className="wire-drop-line" style={{ top: view.line.top, left: view.line.left, width: view.line.width }} />}
      <div className="wire-drag-label" style={{ left: view.x + 14, top: view.y + 10 }}>
        {view.label}
      </div>
    </>,
    document.body,
  );
}
