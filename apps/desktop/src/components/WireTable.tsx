import { ActionIcon, Button, Group, Text, Tooltip } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { IconCopy, IconLayoutRows, IconPlus, IconTrash } from '@tabler/icons-react';
import { useEffect, useRef } from 'react';
import type { Point3 } from '../api/types';
import { newWire, wavelength, wireLength } from '../domain/project';
import { describeSegmentation, formatSegmentation, parseSegmentation } from '../domain/segmentation';
import { insertWires, removeWires } from '../domain/wireEdit';
import { useApp } from '../state/store';
import { NumCell, TextCell, format } from './Cells';

const COORDS: { end: 'start' | 'end'; axis: keyof Point3; label: string }[] = [
  { end: 'start', axis: 'x', label: 'X1' },
  { end: 'start', axis: 'y', label: 'Y1' },
  { end: 'start', axis: 'z', label: 'Z1' },
  { end: 'end', axis: 'x', label: 'X2' },
  { end: 'end', axis: 'y', label: 'Y2' },
  { end: 'end', axis: 'z', label: 'Z2' },
];

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
  const bodyRef = useRef<HTMLTableSectionElement>(null);

  // keep the row selected in the 3D view visible
  useEffect(() => {
    if (selected == null) return;
    bodyRef.current?.children[selected]?.scrollIntoView({ block: 'nearest' });
  }, [selected]);

  const addWire = () => {
    edit((p) => {
      const last = p.antenna.wires.at(-1);
      // continue from the end of the last wire, like drawing in MMANA
      const start = last ? { ...last.end } : { x: 0, y: 0, z: 0 };
      p.antenna.wires.push({ ...(last ?? newWire(start, start)), start, end: { ...start, y: start.y + 1 }, note: '' });
    });
    select(wires.length);
  };

  /** Click on a row number: Shift selects a range, ⌘ / Ctrl adds or removes. */
  const clickRow = (i: number, e: React.MouseEvent) => {
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
              <th key={c.label}>{c.label} (m)</th>
            ))}
            <th>R (mm)</th>
            <th>
              <Tooltip label="分割数。正: 等分、0: 自動等分、-1: 両端テーパ、-2/-3: 始点/終点テーパ、「400,40」: λ/400〜λ/40 のテーパ" multiline w={260}>
                <span>SEG</span>
              </Tooltip>
            </th>
            <th>長さ (m)</th>
            <th className="left">メモ</th>
            <th />
          </tr>
        </thead>
        <tbody ref={bodyRef}>
          {wires.map((w, i) => (
            <tr
              key={i}
              className={selection.includes(i) ? 'selected' : undefined}
              onFocus={() => (selection.includes(i) ? setSelection(selection, i) : select(i))}
            >
              <td className="index num" onClick={(e) => clickRow(i, e)}>
                {i + 1}
              </td>
              {COORDS.map((c) => (
                <td key={c.label} style={{ minWidth: 50 }}>
                  <NumCell
                    ariaLabel={`ワイヤー ${i + 1} ${c.label}`}
                    value={w[c.end][c.axis]}
                    wavelength={lambda}
                    onCommit={(v) => edit((p) => void (p.antenna.wires[i][c.end][c.axis] = v))}
                  />
                </td>
              ))}
              <td style={{ minWidth: 50 }}>
                <NumCell
                  ariaLabel={`ワイヤー ${i + 1} 半径`}
                  value={w.radius}
                  scale={1000}
                  digits={3}
                  wavelength={lambda}
                  onCommit={(v) => edit((p) => void (p.antenna.wires[i].radius = v))}
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
              <td className="readonly num">{format(wireLength(w), 3)}</td>
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
          ))}
        </tbody>
      </table>
      <Group p="xs" justify="space-between">
        <Group gap="xs">
          <Button variant="light" leftSection={<IconPlus size={14} />} onClick={addWire}>
            ワイヤーを追加
          </Button>
          <Tooltip label="つながったワイヤをエレメントとして、幅・全周・間隔などをまとめて変更します" multiline w={260}>
            <Button variant="default" leftSection={<IconLayoutRows size={14} />} onClick={() => setElementEditorOpen(true)}>
              エレメント編集…
            </Button>
          </Tooltip>
        </Group>
        <Text size="xs" c="dimmed">
          数値欄では R（波長 = {format(lambda, 3)} m）や四則演算が使えます
        </Text>
      </Group>
    </>
  );
}
