// The status bar's corner for what happened: the problems of the antenna as
// it is, the messages shown lately, and the undo history to step back to.
import { Badge, Button, Group, Popover, ScrollArea, Stack, Tabs, Text, UnstyledButton } from '@mantine/core';
import { IconAlertTriangle, IconCircleCheck, IconHistory } from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';
import { worst, type Severity } from '../domain/diagnostics';
import { useDiagnostics } from '../state/diagnostics';
import { useMessageLog, type MessageLevel } from '../state/messageLog';
import { useApp } from '../state/store';

type Tab = 'problems' | 'messages' | 'history';

const SEVERITY_COLOR: Record<Severity, string> = { error: 'red', warning: 'yellow', info: 'gray' };
const LEVEL: Record<MessageLevel, { label: string; color: string }> = {
  error: { label: 'エラー', color: 'red' },
  warning: { label: '注意', color: 'yellow' },
  info: { label: 'お知らせ', color: 'gray' },
};

const pad = (n: number) => String(n).padStart(2, '0');
const time = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;

const Empty = ({ children }: { children: string }) => (
  <Group gap={6} p="xs" wrap="nowrap">
    <IconCircleCheck size={16} color="var(--mantine-color-teal-6)" />
    <Text size="sm">{children}</Text>
  </Group>
);

function Problems() {
  const diagnostics = useDiagnostics();
  const result = useApp((s) => (s.resultStale ? null : s.result));
  const setSelection = useApp((s) => s.setSelection);
  // the warnings about the whole antenna name no wires to mark
  const general = result?.warnings.filter((w) => w.wires.length === 0) ?? [];
  if (diagnostics.length === 0 && general.length === 0) return <Empty>問題は見つかっていません</Empty>;
  return (
    <Stack gap={2}>
      {diagnostics.map((d, i) => (
        <UnstyledButton key={i} className="activity-row" onClick={() => setSelection(d.wires)} title="クリックでワイヤーを選択">
          <Group gap={8} wrap="nowrap" align="flex-start">
            <IconAlertTriangle size={14} color={`var(--mantine-color-${SEVERITY_COLOR[d.severity]}-6)`} style={{ flexShrink: 0, marginTop: 2 }} />
            <Text size="xs">{d.message}</Text>
          </Group>
        </UnstyledButton>
      ))}
      {general.map((w, i) => (
        <Group key={`g${i}`} gap={8} wrap="nowrap" align="flex-start" className="activity-row">
          <IconAlertTriangle size={14} color="var(--mantine-color-yellow-6)" style={{ flexShrink: 0, marginTop: 2 }} />
          <Text size="xs">{w.message}</Text>
        </Group>
      ))}
    </Stack>
  );
}

function Messages() {
  const messages = useMessageLog((s) => s.messages);
  if (messages.length === 0) return <Empty>メッセージはありません</Empty>;
  return (
    <Stack gap={2}>
      {[...messages].reverse().map((m, i) => (
        <Group key={i} gap={8} wrap="nowrap" align="flex-start" className="activity-row">
          <Text size="xs" c="dimmed" className="num" style={{ flexShrink: 0 }}>
            {time(m.at)}
          </Text>
          <Badge size="xs" variant="light" color={LEVEL[m.level].color} style={{ flexShrink: 0 }}>
            {LEVEL[m.level].label}
          </Badge>
          <Text size="xs" style={{ minWidth: 0, overflowWrap: 'anywhere' }}>
            {m.title && <b>{m.title}</b>}
            {m.title && m.detail && '：'}
            {m.detail}
          </Text>
        </Group>
      ))}
    </Stack>
  );
}

function History() {
  const past = useApp((s) => s.past);
  const future = useApp((s) => s.future);
  const trimmed = useApp((s) => s.historyTrimmed);
  const travel = useApp((s) => s.travel);
  const current = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    // newer browsers return a promise, which an effect must not
    void current.current?.scrollIntoView({ block: 'nearest' });
  }, []);

  // step k is the state after the k-th change; 0 is where the history starts
  const steps = [trimmed ? 'これより前の履歴は残っていません' : '開いた時点', ...past.map((e) => e.label), ...future.map((e) => e.label)];
  const at = past.length;
  return (
    <Stack gap={2}>
      {steps.map((label, k) => (
        <UnstyledButton
          key={k}
          ref={k === at ? current : undefined}
          className={`activity-row${k === at ? ' current' : ''}`}
          onClick={() => travel(k - at)}
          title={k === at ? '今の状態' : k < at ? 'ここまで戻す' : 'ここまでやり直す'}
        >
          <Text size="xs" c={k > at ? 'dimmed' : undefined} fs={k === 0 ? 'italic' : undefined}>
            {label}
          </Text>
        </UnstyledButton>
      ))}
    </Stack>
  );
}

export function ActivityPanel() {
  const [tab, setTab] = useState<Tab | null>(null);
  const diagnostics = useDiagnostics();
  const steps = useApp((s) => s.past.length);
  const severity = worst(diagnostics);
  const toggle = (t: Tab) => setTab((open) => (open === t ? null : t));

  return (
    <Popover opened={tab != null} onChange={(o) => !o && setTab(null)} position="top-start" shadow="md" withinPortal>
      <Popover.Target>
        <Group gap={2} wrap="nowrap">
          <Button
            size="compact-xs"
            variant={tab === 'problems' || tab === 'messages' ? 'light' : 'subtle'}
            color={severity ? SEVERITY_COLOR[severity] : 'gray'}
            leftSection={<IconAlertTriangle size={13} />}
            onClick={() => toggle('problems')}
          >
            {diagnostics.length}
          </Button>
          <Button
            size="compact-xs"
            variant={tab === 'history' ? 'light' : 'subtle'}
            color="gray"
            leftSection={<IconHistory size={13} />}
            onClick={() => toggle('history')}
          >
            {steps}
          </Button>
        </Group>
      </Popover.Target>
      <Popover.Dropdown p={0}>
        <Tabs value={tab} onChange={(t) => t && setTab(t as Tab)} w={440}>
          <Tabs.List>
            <Tabs.Tab value="problems">警告</Tabs.Tab>
            <Tabs.Tab value="messages">メッセージ</Tabs.Tab>
            <Tabs.Tab value="history">操作の履歴</Tabs.Tab>
          </Tabs.List>
          <ScrollArea.Autosize mah={360} type="auto">
            <div style={{ padding: 6 }}>
              <Tabs.Panel value="problems">
                <Problems />
              </Tabs.Panel>
              <Tabs.Panel value="messages">
                <Messages />
              </Tabs.Panel>
              <Tabs.Panel value="history">
                <History />
              </Tabs.Panel>
            </div>
          </ScrollArea.Autosize>
        </Tabs>
      </Popover.Dropdown>
    </Popover>
  );
}
