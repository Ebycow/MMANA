import { AppShell } from '@mantine/core';
import { useDisclosure, useHotkeys } from '@mantine/hooks';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import { useEffect } from 'react';
import { EngineError, files, isTauri, startupFile, transport } from './api/transport';
import { AppHeader, type FileActions } from './components/AppHeader';
import { ElementEditor } from './components/ElementEditor';
import { InputSettings } from './components/InputSettings';
import { OptimizerDialog } from './components/OptimizerDialog';
import { OptimizerResults } from './components/OptimizerResults';
import { SamplesModal } from './components/SamplesModal';
import { StatusBar } from './components/StatusBar';
import { Workspace } from './components/Workspace';
import { loadSampleBytes, samples as samplesList, type Sample } from './domain/samples';
import { useEditor, type Tool } from './state/editor';
import { useCommandKeys, useInput } from './state/input';
import { useApp } from './state/store';
import { canEditWires, copySelected, cutSelected, deleteSelected, duplicateSelected, paste, selectAll } from './state/wireCommands';

function reportError(e: unknown) {
  const err = e instanceof EngineError ? e : new EngineError('internal', 'エラー', String(e));
  notifications.show({ color: 'red', title: err.title, message: err.detail });
}

/** Asks before discarding unsaved changes. */
function confirmDiscard(): Promise<boolean> {
  if (!useApp.getState().dirty) return Promise.resolve(true);
  return new Promise((resolve) =>
    modals.openConfirmModal({
      title: '保存されていない変更があります',
      children: '変更を破棄して続けますか？',
      labels: { confirm: '破棄して続ける', cancel: 'キャンセル' },
      confirmProps: { color: 'red' },
      onConfirm: () => resolve(true),
      onCancel: () => resolve(false),
    }),
  );
}

function showWarnings(warnings: string[]) {
  if (warnings.length) notifications.show({ color: 'yellow', title: 'ファイルの読み込みで注意があります', message: warnings.join('\n') });
}

let startupSampleHandled = false;

export function App() {
  const [samplesOpen, samples] = useDisclosure(false);
  const s = useApp;

  const actions: FileActions = {
    newFile: async () => (await confirmDiscard()) && s.getState().reset(),
    open: async () => {
      if (!(await confirmDiscard())) return;
      try {
        const opened = await files.open();
        if (!opened) return;
        s.getState().load(opened.parsed, opened.name, opened.path);
        showWarnings(opened.parsed.warnings);
        if (s.getState().autoCalc) s.getState().calculate();
      } catch (e) {
        reportError(e);
      }
    },
    openSample: samples.open,
    save: () => s.getState().save(false).catch(reportError),
    saveAs: () => s.getState().save(true).catch(reportError),
  };

  const pickSample = async (sample: Sample) => {
    samples.close();
    if (!(await confirmDiscard())) return;
    try {
      const parsed = await transport.parseMaa(await loadSampleBytes(sample));
      s.getState().load(parsed, sample.file, null);
      showWarnings(parsed.warnings);
      await s.getState().calculate();
    } catch (e) {
      reportError(e);
    }
  };

  // start-up file: `?sample=3EL20.MAA` (browser) or a path on the command line (desktop)
  useEffect(() => {
    if (startupSampleHandled) return;
    startupSampleHandled = true;
    const name = new URLSearchParams(window.location.search).get('sample');
    const sample = name && samplesList.find((x) => x.file.toLowerCase() === name.toLowerCase());
    if (sample) {
      pickSample(sample);
      return;
    }
    startupFile()
      .then((f) => {
        if (!f) return;
        s.getState().load(f.parsed, f.name, f.path);
        showWarnings(f.parsed.warnings);
        return s.getState().calculate();
      })
      .catch(reportError);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // wire edits from the keyboard, unless a dialog is open
  const wireKey = (f: () => void) => () => {
    if (canEditWires() && !samplesOpen && !useEditor.getState().transformOpen) f();
  };
  // the element editor has its own undo while it is open; a running
  // optimisation applies its result to the antenna it started from
  const historyKey = (f: () => void) => () => {
    if (!s.getState().elementEditorOpen && !s.getState().optimizer.running) f();
  };
  const tool = (t: Tool) => (t === 'select' ? () => useEditor.getState().setTool(t) : wireKey(() => useEditor.getState().setTool(t)));
  useCommandKeys(
    {
      newFile: actions.newFile,
      open: actions.open,
      openSample: actions.openSample,
      save: actions.save,
      saveAs: actions.saveAs,
      undo: historyKey(() => s.getState().undo()),
      redo: historyKey(() => s.getState().redo()),
      copy: copySelected,
      cut: wireKey(cutSelected),
      paste: wireKey(paste),
      duplicate: wireKey(duplicateSelected),
      selectAll,
      delete: wireKey(deleteSelected),
      calculate: () => s.getState().calculate(),
      optimize: () => !s.getState().optimizer.running && s.getState().setOptimizer({ dialogOpen: true }),
      settings: () => useInput.getState().setDialogOpen(true),
      'tool.select': tool('select'),
      'tool.move': tool('move'),
      'tool.rotate': tool('rotate'),
      'tool.wire': tool('wire'),
      'tool.rect': tool('rect'),
      'tool.polygon': tool('polygon'),
      'tool.feed': tool('feed'),
    },
    // calculating must also work while typing in a cell
    ['calculate'],
  );
  useHotkeys([['Escape', () => s.getState().selectWire(null)]]);

  // automatic recalculation
  const project = useApp((st) => st.project);
  const options = useApp((st) => st.options);
  const autoCalc = useApp((st) => st.autoCalc);
  useEffect(() => {
    if (!autoCalc) return;
    const t = setTimeout(() => s.getState().calculate(), 350);
    return () => clearTimeout(t);
  }, [project, options, autoCalc, s]);

  // surface calculation errors
  const calcError = useApp((st) => st.calcError);
  useEffect(() => {
    if (calcError) reportError(calcError);
  }, [calcError]);

  // guard against closing with unsaved changes
  useEffect(() => {
    if (isTauri()) {
      let unlisten: (() => void) | undefined;
      import('@tauri-apps/api/window').then(async ({ getCurrentWindow }) => {
        unlisten = await getCurrentWindow().onCloseRequested(async (event) => {
          if (!useApp.getState().dirty) return;
          const { message } = await import('@tauri-apps/plugin-dialog');
          const buttons = { yes: '保存して終了', no: '保存せずに終了', cancel: 'キャンセル' };
          const choice = await message('保存されていない変更があります。保存してから終了しますか？', { title: 'MMANA', kind: 'warning', buttons });
          if (choice === buttons.no) return;
          // a cancelled or failed save keeps the window open
          if (choice === buttons.yes && (await useApp.getState().save(false).catch((e) => (reportError(e), false)))) return;
          event.preventDefault();
        });
      });
      return () => unlisten?.();
    }
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
      if (useApp.getState().dirty) e.preventDefault();
    };
    window.addEventListener('beforeunload', onBeforeUnload);
    return () => window.removeEventListener('beforeunload', onBeforeUnload);
  }, []);

  // keep the window title in sync
  const fileName = useApp((st) => st.fileName);
  const dirty = useApp((st) => st.dirty);
  useEffect(() => {
    const title = `${dirty ? '● ' : ''}${fileName} — MMANA`;
    document.title = title;
    if (isTauri()) import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().setTitle(title)).catch(() => {});
  }, [fileName, dirty]);

  return (
    <AppShell header={{ height: 52 }} footer={{ height: 26 }} padding={0}>
      <AppShell.Header>
        <AppHeader actions={actions} />
      </AppShell.Header>
      <AppShell.Main style={{ height: '100dvh' }}>
        <div style={{ height: 'calc(100dvh - 52px - 26px)' }}>
          <Workspace />
        </div>
      </AppShell.Main>
      <AppShell.Footer>
        <StatusBar />
      </AppShell.Footer>
      <SamplesModal opened={samplesOpen} onClose={samples.close} onPick={pickSample} />
      <ElementEditor />
      <OptimizerDialog />
      <OptimizerResults />
      <InputSettings />
    </AppShell>
  );
}
