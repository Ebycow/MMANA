import { Anchor, Button, Checkbox, CopyButton, Group, Modal, Stack, Text, Textarea } from '@mantine/core';
import { IconCheck, IconCopy } from '@tabler/icons-react';
import { useMemo, useState } from 'react';
import { isTauri } from '../api/transport';
import { collectDiagnostics } from '../domain/diagnostics';
import { bugReport } from '../domain/report';
import { recentErrors } from '../state/errorLog';
import { useApp } from '../state/store';
import { NEW_ISSUE_URL, versionLine } from '../version';

/** The state as the dialog opens, so what the user reads is what gets copied. */
function snapshot() {
  const s = useApp.getState();
  const calcError = s.calcError && s.calcErrorProject === s.project ? s.calcError : null;
  const current = s.result && !s.resultStale ? s.result : null;
  return {
    version: versionLine(),
    userAgent: navigator.userAgent,
    project: s.project,
    result: s.result,
    resultStale: s.resultStale,
    calcError,
    diagnostics: collectDiagnostics({ wires: s.project.antenna.wires, error: calcError, result: current }),
    errors: recentErrors(),
  };
}

export function ReportModal({ opened, onClose }: { opened: boolean; onClose: () => void }) {
  const [includeAntenna, setIncludeAntenna] = useState(true);
  // taken again each time the dialog opens
  const sources = useMemo(() => (opened ? snapshot() : null), [opened]);
  const text = useMemo(() => (sources ? bugReport({ ...sources, includeAntenna }) : ''), [sources, includeAntenna]);

  return (
    <Modal opened={opened} onClose={onClose} title="不具合報告用の情報" size="xl">
      <Stack gap="sm">
        <Text size="sm">
          下の内容をコピーして、GitHub の Issue に貼り付けてください。Issue は公開されます。載せたくない情報は、貼り付けた後に消してください。
        </Text>
        <Text size="sm" style={{ userSelect: 'text' }}>
          {/* the desktop app cannot open the browser, so it shows the address to copy */}
          {isTauri() ? (
            NEW_ISSUE_URL
          ) : (
            <Anchor href={NEW_ISSUE_URL} target="_blank" rel="noreferrer">
              {NEW_ISSUE_URL}
            </Anchor>
          )}
        </Text>
        <Checkbox
          label="アンテナのデータを含める（同じ計算を再現するのに使います）"
          checked={includeAntenna}
          onChange={(e) => setIncludeAntenna(e.currentTarget.checked)}
        />
        <Textarea value={text} readOnly autosize minRows={12} maxRows={18} styles={{ input: { fontFamily: 'var(--mantine-font-family-monospace)', fontSize: 12 } }} />
        <Group justify="flex-end">
          <CopyButton value={text}>
            {({ copied, copy }) => (
              <Button variant="default" leftSection={copied ? <IconCheck size={15} /> : <IconCopy size={15} />} onClick={copy}>
                {copied ? 'コピーしました' : 'コピー'}
              </Button>
            )}
          </CopyButton>
          <Button onClick={onClose}>閉じる</Button>
        </Group>
      </Stack>
    </Modal>
  );
}
