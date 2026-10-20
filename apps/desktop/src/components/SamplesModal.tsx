import { Modal, ScrollArea, SimpleGrid, TextInput, UnstyledButton, Text } from '@mantine/core';
import { IconSearch } from '@tabler/icons-react';
import { useState } from 'react';
import { samples, type Sample } from '../domain/samples';

export function SamplesModal({ opened, onClose, onPick }: { opened: boolean; onClose: () => void; onPick: (s: Sample) => void }) {
  const [q, setQ] = useState('');
  const shown = samples.filter((s) => s.file.toLowerCase().includes(q.toLowerCase()));
  return (
    <Modal opened={opened} onClose={onClose} title="サンプルアンテナを開く" size="lg">
      <TextInput
        placeholder="ファイル名で絞り込み（例: EL, LOOP, VERT）"
        leftSection={<IconSearch size={14} />}
        value={q}
        onChange={(e) => setQ(e.currentTarget.value)}
        data-autofocus
        mb="sm"
      />
      <ScrollArea h={380}>
        <SimpleGrid cols={4} spacing={6}>
          {shown.map((s) => (
            <UnstyledButton
              key={s.file}
              onClick={() => onPick(s)}
              p="xs"
              style={{ border: '1px solid var(--panel-border)', borderRadius: 8 }}
            >
              <Text size="sm" fw={500} className="num">
                {s.file.replace(/\.MAA$/i, '')}
              </Text>
            </UnstyledButton>
          ))}
        </SimpleGrid>
        {shown.length === 0 && (
          <Text size="sm" c="dimmed" ta="center" mt="lg">
            該当するサンプルはありません
          </Text>
        )}
      </ScrollArea>
    </Modal>
  );
}
