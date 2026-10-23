// The print preview (印刷): the A4 sheets as they will print, with the
// sections and units to choose beside them. Printing hides everything else
// (print.css), so the browser's or the OS's print dialog gets the sheets,
// and saving a PDF goes through that dialog too.
import { Alert, Button, Checkbox, Group, ScrollArea, SegmentedControl, Stack, Text, Title } from '@mantine/core';
import { useHotkeys } from '@mantine/hooks';
import { IconAlertTriangle, IconPrinter } from '@tabler/icons-react';
import { useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import type { ElementEdit } from '../../api/types';
import { transport } from '../../api/transport';
import { collectDiagnostics } from '../../domain/diagnostics';
import { SECTIONS, paginate } from '../../domain/print';
import { wavelength } from '../../domain/project';
import { usePrint, usePrintPolarScale, usePrintUnit, type PrintUnit } from '../../state/print';
import { useApp } from '../../state/store';
import { PLATFORM, VERSION } from '../../version';
import { dateText, printSections, unavailable, type PrintData } from './PrintSections';
import './print.css';

export function PrintView() {
  const open = usePrint((s) => s.open);
  return open ? <Preview /> : null;
}

function Preview() {
  const print = usePrint();
  const unit = usePrintUnit();
  const polarScale = usePrintPolarScale();
  const app = useApp();
  const [elements, setElements] = useState<ElementEdit | null>(null);
  const [printedAt] = useState(() => new Date());
  const close = () => print.setOpen(false);
  useHotkeys([['Escape', close]]);

  const antenna = app.project.antenna;
  useEffect(() => {
    let live = true;
    transport.editElements({ antenna, operations: [] }).then(
      (e) => live && setElements(e),
      () => live && setElements(null),
    );
    return () => {
      live = false;
    };
  }, [antenna]);

  const data: PrintData = useMemo(() => {
    const lambda = wavelength(antenna.frequencyMhz);
    const calcError = app.calcError && app.calcErrorProject === app.project ? app.calcError : null;
    const current = app.result && !app.resultStale ? app.result : null;
    return {
      project: app.project,
      fileName: app.fileName,
      version: `MMANA ${VERSION}（${PLATFORM}）`,
      printedAt,
      result: app.result,
      resultStale: app.resultStale,
      calcError,
      calcFrequencyMhz: app.options.frequencyMhz ?? antenna.frequencyMhz,
      units: { wavelengths: unit === 'λ' && lambda > 0, millimetres: unit === 'mm', lambda },
      polarScale,
      diagnostics: collectDiagnostics({ wires: antenna.wires, error: calcError, result: current }),
      elements,
      sweep: app.sweep,
      resonance: app.resonance,
    };
  }, [app.project, app.fileName, app.result, app.resultStale, app.calcError, app.calcErrorProject, app.options, app.sweep, app.resonance, antenna, unit, polarScale, elements, printedAt]);

  const sections = useMemo(() => printSections(data, print.sections), [data, print.sections]);
  const sheets = useMemo(() => paginate(sections.map((s) => s.block)), [sections]);
  const byKey = new Map(sections.map((s) => [s.block.key, s]));

  // the sheets live in a node of their own under <body>, the only one printed
  const [root] = useState(() => Object.assign(document.createElement('div'), { className: 'print-root' }));
  useEffect(() => {
    document.body.append(root);
    return () => root.remove();
  }, [root]);

  return createPortal(
    <div className="print-view">
        <aside className="print-controls">
          <ScrollArea h="100%" type="auto">
            <Stack gap="md" p="md">
              <Group justify="space-between">
                <Title order={4}>印刷</Title>
                <Text size="xs" c="dimmed">
                  A4 縦・全 {sheets.length} ページ
                </Text>
              </Group>
              <Calculation />
              <Stack gap={8}>
                <Text size="sm" fw={600}>
                  印刷する内容
                </Text>
                {SECTIONS.map((s) => {
                  const why = unavailable(s.id, data);
                  return (
                    <Checkbox
                      key={s.id}
                      label={s.label}
                      description={why ?? s.description}
                      checked={print.sections[s.id] && !why}
                      disabled={why != null}
                      onChange={(e) => print.setSection(s.id, e.currentTarget.checked)}
                    />
                  );
                })}
              </Stack>
              <Stack gap={4}>
                <Text size="sm" fw={600}>
                  長さの単位
                </Text>
                <SegmentedControl
                  value={unit}
                  onChange={(v) => print.setUnit(v as PrintUnit)}
                  data={[
                    { value: 'm', label: 'm' },
                    { value: 'mm', label: 'mm' },
                    { value: 'λ', label: '波長 λ' },
                  ]}
                />
              </Stack>
              <Stack gap={4}>
                <Text size="sm" fw={600}>
                  指向性図の目盛
                </Text>
                <SegmentedControl
                  value={polarScale}
                  onChange={(v) => print.setPolarScale(v as typeof polarScale)}
                  data={[
                    { value: 'mmana', label: 'MMANA' },
                    { value: 'linearDb', label: 'dB リニア' },
                  ]}
                />
              </Stack>
              <Group grow>
                <Button variant="default" onClick={close}>
                  閉じる
                </Button>
                <Button leftSection={<IconPrinter size={15} />} onClick={() => window.print()}>
                  印刷…
                </Button>
              </Group>
              <Text size="xs" c="dimmed">
                PDF にするには、印刷ダイアログで「PDF として保存」を選んでください。
              </Text>
            </Stack>
          </ScrollArea>
        </aside>
        <div className="print-pages">
          {sheets.map((sheet, n) => (
            <section key={n} className="print-sheet">
              <header className="p-running-head">
                <span>{antenna.name || '（名前なし）'}</span>
                <span>
                  {n + 1} / {sheets.length}
                </span>
              </header>
              <div className="p-body">
                {sheet.map((placed) => (
                  <div key={`${placed.key}-${placed.rows?.start ?? 0}`} className="p-block" style={{ height: `${placed.height}mm` }}>
                    {byKey.get(placed.key)!.render(placed.rows)}
                  </div>
                ))}
              </div>
              <footer className="p-running-foot">
                <span>{data.version}</span>
                <span>{data.fileName}</span>
                <span>{dateText(printedAt)}</span>
              </footer>
            </section>
          ))}
        </div>
    </div>,
    root,
  );
}

/** Offers to calculate when the sheets would print no result or an old one. */
function Calculation() {
  const result = useApp((s) => s.result);
  const stale = useApp((s) => s.resultStale);
  const calculating = useApp((s) => s.calculating);
  const calculate = useApp((s) => s.calculate);
  const sweep = useApp((s) => s.sweep);
  const runSweep = useApp((s) => s.runSweep);
  const notes = [
    !result ? 'まだ計算していません。' : stale ? 'アンテナを変更した後、再計算していません。' : null,
    sweep.data && sweep.stale ? '周波数特性も再計算が必要です。' : null,
  ].filter(Boolean);
  if (notes.length === 0) return null;
  return (
    <Alert color="yellow" variant="light" p="xs" icon={<IconAlertTriangle size={16} />}>
      <Stack gap={6}>
        <Text size="xs">{notes.join('')}</Text>
        <Group gap={6}>
          {(!result || stale) && (
            <Button size="compact-xs" loading={calculating} onClick={calculate}>
              計算する
            </Button>
          )}
          {sweep.data && sweep.stale && (
            <Button size="compact-xs" variant="light" loading={sweep.running} onClick={runSweep}>
              周波数特性を計算する
            </Button>
          )}
        </Group>
      </Stack>
    </Alert>
  );
}
