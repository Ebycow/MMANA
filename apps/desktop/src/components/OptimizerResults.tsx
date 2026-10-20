// 最適化結果の一覧 (MMANA's ACalRes dialog): the last trials of the run with
// their figures and patterns; 反映 writes one trial's values back to the
// antenna, so a result the score did not favour can still be chosen.
import { Button, Group, Modal, Select, Stack, Text } from '@mantine/core';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import { useEffect, useMemo, useState } from 'react';
import type { Trial } from '../api/types';
import { EngineError, transport } from '../api/transport';
import { PolarPlot } from '../charts/PolarPlot';
import { strDbl } from '../domain/elements';
import { TRIAL_SORTS, currentTrial, sortTrials, tradeOffText, trialCut, type TrialSort } from '../domain/optimize';
import { useApp } from '../state/store';
import { describeTarget } from './OptimizerDialog';
import { fmtZ } from './ResultsPanel';

export function OptimizerResults() {
  const open = useApp((s) => s.optimizer.resultsOpen && s.optimizer.outcome != null && s.optimizer.outcomeSetup != null);
  return open ? <OptimizerResultsBody /> : null;
}

function OptimizerResultsBody() {
  const outcome = useApp((s) => s.optimizer.outcome)!;
  const setup = useApp((s) => s.optimizer.outcomeSetup)!;
  const project = useApp((s) => s.project);
  const options = useApp((s) => s.options);
  const setOptimizer = useApp((s) => s.setOptimizer);
  const swrRef = project.environment.swrReference.r;
  const [sort, setSort] = useState<TrialSort>('score');
  const trials = useMemo(() => sortTrials(outcome.trials, sort), [outcome.trials, sort]);
  const [selected, setSelected] = useState<Trial | null>(null);
  const [labels, setLabels] = useState<string[]>([]);
  const best = outcome.best?.score ?? 0;
  const sweep = setup.sweepOnly;

  // start at the trial the antenna is at now (`GetCurIndex`)
  useEffect(() => {
    let live = true;
    transport.editOptimizerSetup({ project, options, setup, operations: [] }).then(
      (e) => {
        if (!live) return;
        setLabels(setup.variables.map((v) => describeTarget(v.target, project, e.elements)));
        setSelected((s) => s ?? sortTrials(outcome.trials, 'score')[currentTrial(sortTrials(outcome.trials, 'score'), e.values)] ?? null);
      },
      () => live && setSelected((s) => s ?? outcome.trials[0] ?? null),
    );
    return () => {
      live = false;
    };
    // only when the list opens
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const close = () => setOptimizer({ resultsOpen: false });
  const apply = () => {
    if (!selected) return;
    modals.openConfirmModal({
      title: 'この結果を反映しますか？',
      children: <Text size="sm">アンテナ定義がこの時の状態に更新されます（「元に戻す」で戻せます）。</Text>,
      labels: { confirm: '反映', cancel: 'キャンセル' },
      onConfirm: async () => {
        try {
          await useApp.getState().applyTrial(selected.values);
          close();
        } catch (e) {
          const err = e instanceof EngineError ? e : new EngineError('internal', 'エラー', String(e));
          notifications.show({ color: 'red', title: '反映できません', message: err.detail });
        }
      },
    });
  };

  const peak = selected ? Math.max(...selected.horizontalPattern, ...selected.verticalPattern) : 0;

  return (
    <Modal opened onClose={close} title="最適化結果の一覧" size="min(1200px, 96vw)">
      <Group justify="space-between" mb="xs">
        <Text size="sm" c="dimmed">
          {tradeOffText(setup)}・最新 {outcome.trials.length} 件
        </Text>
        <Select
          size="xs"
          w={150}
          label="順序"
          allowDeselect={false}
          value={sort}
          onChange={(v) => v && setSort(v as TrialSort)}
          data={TRIAL_SORTS}
        />
      </Group>
      <div style={{ display: 'flex', gap: 12, height: '62vh', minHeight: 320 }}>
        <div style={{ flex: 1.2, minWidth: 0, overflow: 'auto', border: '1px solid var(--panel-border)', borderRadius: 8 }}>
          <table className="grid-table" aria-label="最適化結果">
            <thead>
              <tr>
                <th style={{ textAlign: 'center' }}>No.</th>
                <th>Z (Ω)</th>
                <th>SWR ({strDbl(swrRef)})</th>
                <th>Ga (dBi)</th>
                <th>F/B (dB)</th>
                <th>仰角 (°)</th>
                {sweep ? setup.variables.map((_, i) => <th key={i} title={labels[i]}>V{i + 1}</th>) : <th title="最良の評価値を 1 とした値">評価値</th>}
              </tr>
            </thead>
            <tbody>
              {trials.map((t, i) => (
                <tr
                  key={i}
                  className={t === selected ? 'selected' : undefined}
                  onClick={() => setSelected(t)}
                  style={{ cursor: 'pointer' }}
                  aria-selected={t === selected}
                >
                  <td className="index num">{i + 1}</td>
                  <td className="readonly num">{fmtZ(t.impedance.re, t.impedance.im)}</td>
                  <td className="readonly num">{t.swr >= 99999 ? '∞' : t.swr.toFixed(2)}</td>
                  <td className="readonly num">{t.gainDbi.toFixed(2)}</td>
                  <td className="readonly num">{t.fbDb.toFixed(2)}</td>
                  <td className="readonly num">{t.elevationDeg.toFixed(1)}</td>
                  {sweep ? (
                    t.values.map((v, k) => (
                      <td key={k} className="readonly num">
                        {strDbl(v)}
                      </td>
                    ))
                  ) : (
                    <td className="readonly num">{best !== 0 ? (t.score / best).toFixed(4) : strDbl(t.score)}</td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <Stack gap="xs" style={{ flex: 1, minWidth: 300 }}>
          {selected ? (
            <>
              <div style={{ display: 'flex', gap: 12, flex: 1, minHeight: 0 }}>
                <PolarPlot kind="horizontal" cut={trialCut(selected.horizontalPattern)} maxDb={peak} title="水平面" subtitle="2° 分解能" totalOnly />
                <PolarPlot kind="vertical" cut={trialCut(selected.verticalPattern)} maxDb={peak} title="垂直面" subtitle="X-Z 面" totalOnly />
              </div>
              <Text size="xs" c="dimmed">
                最大 {peak.toFixed(2)} dBi を 0 dB として表示
              </Text>
              <div style={{ maxHeight: 160, overflow: 'auto' }}>
                <table className="grid-table" aria-label="この結果の変数">
                  <tbody>
                    {selected.values.map((v, i) => (
                      <tr key={i}>
                        <td className="index num">{i + 1}</td>
                        <td className="left" style={{ padding: '0 6px' }}>
                          {labels[i]}
                        </td>
                        <td className="readonly num">{strDbl(v)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          ) : (
            <Text size="sm" c="dimmed">
              結果を選んでください
            </Text>
          )}
        </Stack>
      </div>
      <Group justify="space-between" mt="sm">
        <Text size="xs" c="dimmed">
          最適化中に試した最新 {outcome.trials.length} 件の結果です。数値は 1 番のバンドのものです。
        </Text>
        <Group gap="xs">
          <Button variant="default" onClick={close}>
            閉じる
          </Button>
          <Button disabled={!selected} onClick={apply}>
            反映
          </Button>
        </Group>
      </Group>
    </Modal>
  );
}
