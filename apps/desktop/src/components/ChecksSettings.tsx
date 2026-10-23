// The dialog that chooses which problems the wire table and the 3D view mark.
import { Badge, Group, Modal, Stack, Switch, Text } from '@mantine/core';
import { RULES, canHide, type Severity } from '../domain/diagnostics';
import { useChecks } from '../state/diagnostics';

const SEVERITY: Record<Severity, { label: string; color: string }> = {
  error: { label: 'エラー', color: 'red' },
  warning: { label: '警告', color: 'yellow' },
  info: { label: '情報', color: 'gray' },
};

export function ChecksSettings() {
  const open = useChecks((s) => s.dialogOpen);
  const setOpen = useChecks((s) => s.setDialogOpen);
  const hidden = useChecks((s) => s.hidden);
  const setHidden = useChecks((s) => s.setHidden);
  return (
    <Modal opened={open} onClose={() => setOpen(false)} title="警告の表示" size="md">
      <Stack gap="sm">
        <Text size="xs" c="dimmed">
          問題のあるワイヤーは、ワイヤー表の番号と 3D 表示の色で示します。エラーは計算が止まる原因なので、いつも表示します。
        </Text>
        {RULES.map((r) => (
          <Group key={r.id} gap="sm" wrap="nowrap" align="flex-start">
            <Switch
              mt={2}
              aria-label={r.label}
              checked={!canHide(r.id) || !hidden.includes(r.id)}
              disabled={!canHide(r.id)}
              onChange={(e) => setHidden(r.id, !e.currentTarget.checked)}
            />
            <div style={{ flex: 1 }}>
              <Group gap={6}>
                <Text size="sm">{r.label}</Text>
                <Badge size="xs" variant="light" color={SEVERITY[r.severity].color}>
                  {SEVERITY[r.severity].label}
                </Badge>
              </Group>
              <Text size="xs" c="dimmed">
                {r.description}
              </Text>
            </div>
          </Group>
        ))}
      </Stack>
    </Modal>
  );
}
