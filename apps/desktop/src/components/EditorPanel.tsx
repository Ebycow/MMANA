import { Badge, Group, ScrollArea, Tabs } from '@mantine/core';
import { IconAdjustmentsHorizontal, IconBolt, IconPlanet, IconVectorSpline } from '@tabler/icons-react';
import { useApp } from '../state/store';
import { AdvancedForm } from './AdvancedForm';
import { EnvironmentForm } from './EnvironmentForm';
import { FieldNumber, FieldText } from './Fields';
import { SourcesLoads } from './SourcesLoads';
import { WireTable } from './WireTable';

export function EditorPanel() {
  const ant = useApp((s) => s.project.antenna);
  const edit = useApp((s) => s.edit);

  return (
    <section className="panel">
      <Group className="panel-header" gap="sm" wrap="nowrap" align="end" py={10}>
        <FieldText
          label="アンテナ名"
          style={{ flex: 1 }}
          value={ant.name}
          onCommit={(v) => edit((p) => void (p.antenna.name = v))}
        />
        <FieldNumber
          label="設計周波数 (MHz)"
          w={150}
          value={ant.frequencyMhz}
          min={0.001}
          decimalScale={6}
          onCommit={(v) => edit((p) => void (p.antenna.frequencyMhz = v))}
        />
      </Group>
      <Tabs defaultValue="wires" keepMounted={false} style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }}>
        <Tabs.List px="xs">
          <Tabs.Tab value="wires" leftSection={<IconVectorSpline size={14} />}>
            ワイヤー
            <Badge size="xs" variant="light" color="gray" ml={6}>
              {ant.wires.length}
            </Badge>
          </Tabs.Tab>
          <Tabs.Tab value="feed" leftSection={<IconBolt size={14} />}>
            給電点・負荷
          </Tabs.Tab>
          <Tabs.Tab value="env" leftSection={<IconPlanet size={14} />}>
            環境
          </Tabs.Tab>
          <Tabs.Tab value="advanced" leftSection={<IconAdjustmentsHorizontal size={14} />}>
            詳細
          </Tabs.Tab>
        </Tabs.List>
        <ScrollArea style={{ flex: 1 }} type="auto">
          <Tabs.Panel value="wires">
            <WireTable />
          </Tabs.Panel>
          <Tabs.Panel value="feed">
            <SourcesLoads />
          </Tabs.Panel>
          <Tabs.Panel value="env">
            <EnvironmentForm />
          </Tabs.Panel>
          <Tabs.Panel value="advanced">
            <AdvancedForm />
          </Tabs.Panel>
        </ScrollArea>
      </Tabs>
    </section>
  );
}
