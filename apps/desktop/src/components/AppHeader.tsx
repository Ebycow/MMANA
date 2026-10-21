import { ActionIcon, Button, Divider, Group, Kbd, Menu, Switch, Text, Tooltip, useMantineColorScheme } from '@mantine/core';
import {
  IconAntenna,
  IconArrowBackUp,
  IconArrowForwardUp,
  IconCheck,
  IconChevronDown,
  IconDeviceFloppy,
  IconFile,
  IconFileImport,
  IconFolderOpen,
  IconListDetails,
  IconMoon,
  IconPlayerPlay,
  IconSun,
  IconTarget,
} from '@tabler/icons-react';
import { useLayout, type PaneId } from '../state/layout';
import { useApp } from '../state/store';

export interface FileActions {
  newFile: () => void;
  open: () => void;
  openSample: () => void;
  save: () => void;
  saveAs: () => void;
}

const MOD = navigator.platform.toLowerCase().includes('mac') ? '⌘' : 'Ctrl';

const PANES: [PaneId, string][] = [
  ['editor', 'アンテナ（ワイヤー表など）'],
  ['view', 'アンテナ形状（3D）'],
  ['results', '計算結果'],
];

export function AppHeader({ actions }: { actions: FileActions }) {
  const fileName = useApp((s) => s.fileName);
  const dirty = useApp((s) => s.dirty);
  const canUndo = useApp((s) => s.past.length > 0);
  const canRedo = useApp((s) => s.future.length > 0);
  const undo = useApp((s) => s.undo);
  const redo = useApp((s) => s.redo);
  const calculate = useApp((s) => s.calculate);
  const calculating = useApp((s) => s.calculating);
  const autoCalc = useApp((s) => s.autoCalc);
  const setAutoCalc = useApp((s) => s.setAutoCalc);
  const optimizing = useApp((s) => s.optimizer.running);
  const hasTrials = useApp((s) => (s.optimizer.outcome?.trials.length ?? 0) > 0);
  const setOptimizer = useApp((s) => s.setOptimizer);
  const shown = useLayout((s) => s.shown);
  const setShown = useLayout((s) => s.setShown);
  const shownCount = PANES.filter(([id]) => shown[id]).length;
  const { colorScheme, setColorScheme } = useMantineColorScheme();
  const dark = colorScheme === 'dark';

  return (
    <Group h="100%" px="md" gap="sm" wrap="nowrap">
      <Group gap={8} wrap="nowrap">
        <IconAntenna size={22} stroke={1.8} color="var(--mantine-primary-color-filled)" />
        <Text fw={700} size="md">
          MMANA
        </Text>
      </Group>
      <Divider orientation="vertical" />
      <Menu position="bottom-start" shadow="md" width={240}>
        <Menu.Target>
          <Button variant="subtle" color="gray" rightSection={<IconChevronDown size={14} />}>
            ファイル
          </Button>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Item leftSection={<IconFile size={14} />} onClick={actions.newFile}>
            新規作成
          </Menu.Item>
          <Menu.Item leftSection={<IconFolderOpen size={14} />} rightSection={<Kbd size="xs">{MOD} O</Kbd>} onClick={actions.open}>
            開く…
          </Menu.Item>
          <Menu.Item leftSection={<IconFileImport size={14} />} onClick={actions.openSample}>
            サンプルを開く…
          </Menu.Item>
          <Menu.Divider />
          <Menu.Item leftSection={<IconDeviceFloppy size={14} />} rightSection={<Kbd size="xs">{MOD} S</Kbd>} onClick={actions.save}>
            保存
          </Menu.Item>
          <Menu.Item rightSection={<Kbd size="xs">{MOD} ⇧ S</Kbd>} onClick={actions.saveAs}>
            名前を付けて保存…
          </Menu.Item>
        </Menu.Dropdown>
      </Menu>
      <Menu position="bottom-start" shadow="md" width={240} closeOnItemClick={false}>
        <Menu.Target>
          <Button variant="subtle" color="gray" rightSection={<IconChevronDown size={14} />}>
            表示
          </Button>
        </Menu.Target>
        <Menu.Dropdown>
          {PANES.map(([id, label]) => (
            <Menu.Item
              key={id}
              leftSection={<IconCheck size={14} style={{ visibility: shown[id] ? 'visible' : 'hidden' }} />}
              // one panel always stays
              disabled={shown[id] && shownCount === 1}
              onClick={() => setShown(id, !shown[id])}
            >
              {label}
            </Menu.Item>
          ))}
        </Menu.Dropdown>
      </Menu>
      <Group gap={2} wrap="nowrap">
        <Tooltip label={`元に戻す (${MOD} Z)`}>
          <ActionIcon aria-label="元に戻す" disabled={!canUndo} onClick={undo}>
            <IconArrowBackUp size={17} />
          </ActionIcon>
        </Tooltip>
        <Tooltip label={`やり直す (${MOD} ⇧ Z)`}>
          <ActionIcon aria-label="やり直す" disabled={!canRedo} onClick={redo}>
            <IconArrowForwardUp size={17} />
          </ActionIcon>
        </Tooltip>
      </Group>
      <Text size="sm" c="dimmed" truncate style={{ flex: 1, minWidth: 0 }} ta="center">
        {fileName}
        {dirty && (
          <Text span c="dimmed" ml={6} title="未保存の変更があります">
            ● 未保存
          </Text>
        )}
      </Text>
      <Group gap={2} wrap="nowrap">
        <Tooltip label="寸法・集中定数・地上高などを変えながら、利得・F/B・SWR などの目標に近づけます">
          <Button
            size="sm"
            variant="default"
            leftSection={<IconTarget size={15} />}
            loading={optimizing}
            onClick={() => setOptimizer({ dialogOpen: true })}
          >
            最適化
          </Button>
        </Tooltip>
        <Tooltip label="最適化結果の一覧">
          <ActionIcon aria-label="最適化結果の一覧" disabled={!hasTrials || optimizing} onClick={() => setOptimizer({ resultsOpen: true })}>
            <IconListDetails size={17} />
          </ActionIcon>
        </Tooltip>
      </Group>
      <Tooltip label="編集するたびに自動で再計算します">
        <Switch label="自動計算" checked={autoCalc} onChange={(e) => setAutoCalc(e.currentTarget.checked)} />
      </Tooltip>
      <Button size="sm" leftSection={<IconPlayerPlay size={15} />} loading={calculating} onClick={calculate} rightSection={<Kbd size="xs">{MOD} ↵</Kbd>}>
        計算
      </Button>
      <Tooltip label={dark ? 'ライトモード' : 'ダークモード'}>
        <ActionIcon aria-label="表示モードを切り替え" onClick={() => setColorScheme(dark ? 'light' : 'dark')}>
          {dark ? <IconSun size={17} /> : <IconMoon size={17} />}
        </ActionIcon>
      </Tooltip>
    </Group>
  );
}
