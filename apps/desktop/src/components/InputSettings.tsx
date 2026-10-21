// The dialog that reassigns keyboard shortcuts and what the mouse does in the
// 3D view.
import { ActionIcon, Alert, Button, Chip, Group, Kbd, Modal, ScrollArea, SegmentedControl, Slider, Stack, Switch, Table, Tabs, Text, TextInput, Title, Tooltip } from '@mantine/core';
import { IconArrowBackUp, IconInfoCircle, IconKeyboard, IconMouse, IconPlus, IconSearch, IconX } from '@tabler/icons-react';
import { Fragment, useEffect, useState } from 'react';
import {
  COMMANDS,
  DEFAULT_BINDINGS,
  canonical,
  formatKey,
  fromEvent,
  heldLabel,
  type CameraAction,
  type CommandId,
  type HeldKey,
  type MouseButtonName,
  type MouseSettings,
} from '../domain/shortcuts';
import { useInput, useKeyText } from '../state/input';

const GROUPS = [...new Set(COMMANDS.map((c) => c.group))];
const label = (id: CommandId) => COMMANDS.find((c) => c.id === id)!.label;
const sameKeys = (a: string[], b: string[]) => a.length === b.length && a.every((k, i) => canonical(k) === canonical(b[i]));

/** The first key of a command, if it has one. */
export function KeyHint({ id }: { id: CommandId }) {
  const text = useKeyText(id);
  return text ? <Kbd size="xs">{text}</Kbd> : null;
}

export function InputSettings() {
  const open = useInput((s) => s.dialogOpen);
  const setOpen = useInput((s) => s.setDialogOpen);
  return (
    <Modal opened={open} onClose={() => setOpen(false)} title="キーボードとマウスの設定" size="lg">
      <Tabs defaultValue="keys" keepMounted={false}>
        <Tabs.List mb="sm">
          <Tabs.Tab value="keys" leftSection={<IconKeyboard size={14} />}>
            キーボード
          </Tabs.Tab>
          <Tabs.Tab value="mouse" leftSection={<IconMouse size={14} />}>
            マウス
          </Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="keys">
          <KeySettings />
        </Tabs.Panel>
        <Tabs.Panel value="mouse">
          <MouseSettingsForm />
        </Tabs.Panel>
      </Tabs>
    </Modal>
  );
}

function KeySettings() {
  const bindings = useInput((s) => s.bindings);
  const addKey = useInput((s) => s.addKey);
  const removeKey = useInput((s) => s.removeKey);
  const resetKeys = useInput((s) => s.resetKeys);
  const [q, setQ] = useState('');
  const [capturing, setCapturing] = useState<CommandId | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // the next key press is the new shortcut; it goes nowhere else
  useEffect(() => {
    if (!capturing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.isComposing) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Escape' && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) return setCapturing(null);
      const key = fromEvent(e);
      if (!key) return;
      const taken = addKey(capturing, key);
      setNotice(taken.length ? `${formatKey(key)} を「${taken.map(label).join('」「')}」から「${label(capturing)}」に移しました。` : null);
      setCapturing(null);
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [capturing, addKey]);

  const query = q.trim().toLowerCase();
  const shown = COMMANDS.filter(
    (c) => !query || c.label.toLowerCase().includes(query) || c.group.toLowerCase().includes(query) || bindings[c.id].some((k) => formatKey(k).toLowerCase().includes(query)),
  );

  return (
    <Stack gap="sm">
      <TextInput
        placeholder="操作名・キーで絞り込み"
        leftSection={<IconSearch size={14} />}
        value={q}
        onChange={(e) => setQ(e.currentTarget.value)}
        size="xs"
      />
      {notice && (
        <Alert color="yellow" p="xs" withCloseButton onClose={() => setNotice(null)}>
          <Text size="xs">{notice}</Text>
        </Alert>
      )}
      <ScrollArea h={380} type="auto">
        <Table verticalSpacing={4} highlightOnHover>
          <Table.Tbody>
            {GROUPS.map((group) => {
              const rows = shown.filter((c) => c.group === group);
              if (rows.length === 0) return null;
              return (
                <Fragment key={group}>
                  <Table.Tr>
                    <Table.Td colSpan={3} pt="sm">
                      <Text size="xs" fw={600} c="dimmed">
                        {group}
                      </Text>
                    </Table.Td>
                  </Table.Tr>
                  {rows.map((c) => {
                    const keys = bindings[c.id];
                    const changed = !sameKeys(keys, DEFAULT_BINDINGS[c.id]);
                    return (
                      <Table.Tr key={c.id}>
                        <Table.Td w="38%">
                          <Text size="sm">{c.label}</Text>
                        </Table.Td>
                        <Table.Td>
                          <Group gap={6}>
                            {keys.map((k) => (
                              <Group key={k} gap={2} wrap="nowrap">
                                <Kbd size="xs">{formatKey(k)}</Kbd>
                                <ActionIcon size="xs" variant="subtle" color="gray" aria-label={`${formatKey(k)} を外す`} onClick={() => removeKey(c.id, k)}>
                                  <IconX size={11} />
                                </ActionIcon>
                              </Group>
                            ))}
                            {capturing === c.id ? (
                              <Button size="compact-xs" variant="light" autoFocus onBlur={() => setCapturing(null)} onClick={() => setCapturing(null)}>
                                キーを押してください（Esc で取り消し）
                              </Button>
                            ) : (
                              <Tooltip label="キーを追加">
                                <ActionIcon size="sm" variant="subtle" color="gray" aria-label={`${c.label}にキーを追加`} onClick={() => setCapturing(c.id)}>
                                  <IconPlus size={13} />
                                </ActionIcon>
                              </Tooltip>
                            )}
                          </Group>
                        </Table.Td>
                        <Table.Td w={32}>
                          {changed && (
                            <Tooltip label="既定に戻す">
                              <ActionIcon size="sm" variant="subtle" color="gray" aria-label={`${c.label}を既定に戻す`} onClick={() => resetKeys(c.id)}>
                                <IconArrowBackUp size={13} />
                              </ActionIcon>
                            </Tooltip>
                          )}
                        </Table.Td>
                      </Table.Tr>
                    );
                  })}
                </Fragment>
              );
            })}
          </Table.Tbody>
        </Table>
      </ScrollArea>
      <Group justify="space-between" align="flex-start" wrap="nowrap">
        <Text size="xs" c="dimmed">
          Esc（取り消し・選択の解除）、表の中の Enter、描画中の数字・Enter・Backspace は変えられません。
          文字を入力している間は、計算以外のショートカットは働きません。
        </Text>
        <Button size="xs" variant="default" style={{ flexShrink: 0 }} onClick={() => (resetKeys(), setNotice(null))}>
          すべて既定に戻す
        </Button>
      </Group>
    </Stack>
  );
}

const BUTTONS: [MouseButtonName, string][] = [
  ['left', '左ボタン'],
  ['middle', '中ボタン（ホイール）'],
  ['right', '右ボタン'],
];

const ACTIONS: { value: CameraAction; label: string }[] = [
  { value: 'rotate', label: '回転' },
  { value: 'pan', label: '平行移動' },
  { value: 'zoom', label: 'ズーム' },
  { value: 'none', label: 'なし' },
];

const HELD_KEYS: HeldKey[] = ['shift', 'alt', 'mod'];

const MODIFIER_ROWS: { setting: 'boxSelect' | 'addToSelection' | 'constrain' | 'followToggle'; label: string; help: string }[] = [
  { setting: 'addToSelection', label: '選択に追加・外す', help: 'ワイヤーをクリック・ダブルクリックするときに押す' },
  { setting: 'boxSelect', label: '範囲選択', help: '空いた所を左ボタンでドラッグするときに押す' },
  { setting: 'constrain', label: '1 軸に固定・15° 刻み', help: 'ドラッグ中は 1 軸に固定（端点は吸着しない）、描画中は 15° 刻み' },
  { setting: 'followToggle', label: '「つながりを保つ」を切り替え', help: 'ワイヤー・端点をドラッグしている間だけ逆にする' },
];

function MouseSettingsForm() {
  const mouse = useInput((s) => s.mouse);
  const setMouse = useInput((s) => s.setMouse);
  const resetMouse = useInput((s) => s.resetMouse);
  const setButton = (b: MouseButtonName, a: CameraAction) => setMouse({ buttons: { ...mouse.buttons, [b]: a } });
  const leftOnEmpty = mouse.buttons.left;

  return (
    <ScrollArea h={460} type="auto" offsetScrollbars>
      <Stack gap="md">
        <div>
          <Title order={6} mb={4}>
            カメラ（3D ビュー）
          </Title>
          <Text size="xs" c="dimmed" mb="xs">
            空いた所をドラッグしたときの動き。左ボタンはワイヤーや端点の上では選択と移動に使います。
          </Text>
          <Stack gap={6}>
            {BUTTONS.map(([b, name]) => (
              <Group key={b} justify="space-between" wrap="nowrap">
                <Text size="sm">{name}</Text>
                <SegmentedControl size="xs" w={300} value={mouse.buttons[b]} onChange={(v) => setButton(b, v as CameraAction)} data={ACTIONS} />
              </Group>
            ))}
          </Stack>
          {(leftOnEmpty === 'rotate' || leftOnEmpty === 'pan') && (
            <Text size="xs" c="dimmed" mt={6}>
              左ボタンで Shift・{heldLabel('mod')} を押しながらドラッグすると{leftOnEmpty === 'rotate' ? '平行移動' : '回転'}になります（範囲選択のキーを除く）。
            </Text>
          )}
          {!Object.values(mouse.buttons).includes('rotate') && (
            <Alert color="yellow" p="xs" mt={6} icon={<IconInfoCircle size={16} />}>
              <Text size="xs">回転に割り当てたボタンがありません。視点は右下のキューブでも変えられます。</Text>
            </Alert>
          )}
        </div>

        <div>
          <Title order={6} mb="xs">
            ホイール・速さ
          </Title>
          <Stack gap="xs">
            <Switch label="ズームの向きを逆にする（ホイール・ドラッグとも）" checked={mouse.reverseZoom} onChange={(e) => setMouse({ reverseZoom: e.currentTarget.checked })} />
            <Switch label="ポインターの位置に向かってズーム" checked={mouse.zoomToCursor} onChange={(e) => setMouse({ zoomToCursor: e.currentTarget.checked })} />
            <SpeedSlider label="ズームの速さ" value={mouse.zoomSpeed} onChange={(zoomSpeed) => setMouse({ zoomSpeed })} />
            <SpeedSlider label="回転の速さ" value={mouse.rotateSpeed} onChange={(rotateSpeed) => setMouse({ rotateSpeed })} />
          </Stack>
        </div>

        <div>
          <Title order={6} mb={4}>
            押しながら操作するキー
          </Title>
          <Text size="xs" c="dimmed" mb="xs">
            複数選ぶと、どれを押しても働きます。何も選ばないとその操作は使えません。
          </Text>
          <Stack gap="xs">
            {MODIFIER_ROWS.map((r) => (
              <Group key={r.setting} justify="space-between" wrap="nowrap" align="flex-start">
                <div>
                  <Text size="sm">{r.label}</Text>
                  <Text size="xs" c="dimmed">
                    {r.help}
                  </Text>
                </div>
                <Chip.Group multiple value={mouse[r.setting]} onChange={(v) => setMouse({ [r.setting]: HELD_KEYS.filter((k) => v.includes(k)) } as Partial<MouseSettings>)}>
                  <Group gap={4} wrap="nowrap">
                    {HELD_KEYS.map((k) => (
                      <Chip key={k} value={k} size="xs">
                        {heldLabel(k)}
                      </Chip>
                    ))}
                  </Group>
                </Chip.Group>
              </Group>
            ))}
          </Stack>
        </div>

        <Group justify="flex-end">
          <Button size="xs" variant="default" onClick={resetMouse}>
            マウスの設定を既定に戻す
          </Button>
        </Group>
      </Stack>
    </ScrollArea>
  );
}

function SpeedSlider({ label, value, onChange }: { label: string; value: number; onChange: (v: number) => void }) {
  return (
    <Group justify="space-between" wrap="nowrap">
      <Text size="sm">{label}</Text>
      <Slider
        w={300}
        min={0.2}
        max={3}
        step={0.1}
        value={value}
        onChange={onChange}
        label={(v) => `× ${v.toFixed(1)}`}
        marks={[{ value: 1, label: '標準' }]}
        mb="sm"
      />
    </Group>
  );
}
