import { Button, CopyButton, Group, Modal, Stack, Table, Text } from '@mantine/core';
import { IconAntenna, IconCheck, IconCopy } from '@tabler/icons-react';
import { BUILT_AT, PLATFORM, VERSION, formatBuiltAt, versionLine } from '../version';

export function AboutModal({ opened, onClose }: { opened: boolean; onClose: () => void }) {
  return (
    <Modal opened={opened} onClose={onClose} title="MMANAについて" size="md">
      <Stack gap="md">
        <Group gap="sm" wrap="nowrap">
          <IconAntenna size={40} stroke={1.6} color="var(--mantine-primary-color-filled)" />
          <div>
            <Text fw={700} size="lg">
              MMANA
            </Text>
            <Text size="sm" c="dimmed">
              JE3HHT 森 誠氏の MMANA（MININEC3 ベースのアンテナ解析ソフト）を Rust に移植したものです。
            </Text>
          </div>
        </Group>
        <Table withRowBorders={false} verticalSpacing={4}>
          <Table.Tbody>
            <Table.Tr>
              <Table.Td c="dimmed" w={110}>
                バージョン
              </Table.Td>
              <Table.Td className="num" style={{ userSelect: 'text' }}>
                {VERSION}
              </Table.Td>
            </Table.Tr>
            <Table.Tr>
              <Table.Td c="dimmed">動作環境</Table.Td>
              <Table.Td>{PLATFORM}</Table.Td>
            </Table.Tr>
            <Table.Tr>
              <Table.Td c="dimmed">ビルド日時</Table.Td>
              <Table.Td className="num">{formatBuiltAt(BUILT_AT)}</Table.Td>
            </Table.Tr>
            <Table.Tr>
              <Table.Td c="dimmed">ライセンス</Table.Td>
              <Table.Td>LGPL-3.0-or-later</Table.Td>
            </Table.Tr>
            <Table.Tr>
              <Table.Td c="dimmed">ソースコード</Table.Td>
              <Table.Td style={{ userSelect: 'text' }}>github.com/Ebycow/MMMANA</Table.Td>
            </Table.Tr>
          </Table.Tbody>
        </Table>
        <Text size="xs" c="dimmed">
          オリジナルの MMANA の LGPL を受け継いでいます。
          <br />
          Copyright 1999-2013 Makoto Mori (JE3HHT), Nobuyuki Oba (JA7UDE)
        </Text>
        <Group justify="flex-end">
          <CopyButton value={versionLine()}>
            {({ copied, copy }) => (
              <Button variant="default" leftSection={copied ? <IconCheck size={15} /> : <IconCopy size={15} />} onClick={copy}>
                {copied ? 'コピーしました' : 'バージョン情報をコピー'}
              </Button>
            )}
          </CopyButton>
          <Button onClick={onClose}>閉じる</Button>
        </Group>
      </Stack>
    </Modal>
  );
}
