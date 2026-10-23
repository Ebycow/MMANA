import { describe, expect, it } from 'vitest';
import { EngineError } from '../api/transport';
import type { CalcResult, Project } from '../api/types';
import { newProject } from '../domain/project';
import { bugReport, compactJson, type ReportSources } from '../domain/report';

const result = (patch: Partial<CalcResult> = {}) =>
  ({
    frequencyMhz: 14.05,
    impedance: { re: 72.4, im: -3.21 },
    swr: 1.46,
    gainDbi: 7.123,
    fbDb: 12.3,
    elevationDeg: 15,
    warnings: [],
    ...patch,
  }) as unknown as CalcResult;

const sources = (patch: Partial<ReportSources> = {}): ReportSources => ({
  version: 'MMANA v0.2.0-43-g060248a (Web, built 2026-10-23T13:57:00.000Z)',
  userAgent: 'Mozilla/5.0',
  project: newProject(),
  result: null,
  resultStale: false,
  calcError: null,
  diagnostics: [],
  messages: [],
  includeAntenna: true,
  ...patch,
});

describe('the bug report', () => {
  it('names the build and the browser', () => {
    expect(bugReport(sources())).toContain('### 環境\n\n- MMANA v0.2.0-43-g060248a (Web, built 2026-10-23T13:57:00.000Z)\n- Mozilla/5.0');
  });

  it('gives the result as the results panel shows it', () => {
    const text = bugReport(sources({ result: result() }));
    expect(text).toContain('- インピーダンス 72.40 − j3.21 Ω');
    expect(text).toContain('- SWR 1.46（50 Ω）');
    expect(text).toContain('- 利得 7.12 dBi、F/B 12.30 dB、打ち上げ角 15.0°');
  });

  it('says when the result is not of the antenna as it is', () => {
    expect(bugReport(sources({ result: result(), resultStale: true }))).toContain('アンテナを変更した後、再計算していません。');
  });

  it('gives a calculation error in place of the result', () => {
    const calcError = new EngineError('calculation-failed', '計算できません', 'wires 1 and 2 overlap', 'overlapping-wires', [1, 2]);
    expect(bugReport(sources({ result: result(), calcError }))).toContain('計算できませんでした：計算できません（wires 1 and 2 overlap）');
  });

  it('lists the warnings about the whole antenna with the result, the others as problems', () => {
    const warnings = [
      { code: 'closeWires', message: 'close', wires: [1, 2] },
      { code: 'negligiblePower', message: '電力が小さすぎます', wires: [] },
    ] as unknown as CalcResult['warnings'];
    const text = bugReport(
      sources({ result: result({ warnings }), diagnostics: [{ rule: 'gap', severity: 'warning', wires: [0, 1], message: 'ワイヤー 1 と 2 の端がずれています' }] }),
    );
    expect(text).toContain('- 注意：電力が小さすぎます');
    expect(text).not.toContain('注意：close');
    expect(text).toContain('### 警告\n\n- 警告：ワイヤー 1 と 2 の端がずれています');
  });

  it('keeps each message on one line, with its level', () => {
    const messages = [
      { at: new Date(2026, 9, 23, 9, 5, 7), level: 'error' as const, title: '保存できません', detail: 'line one\n  line two' },
      { at: new Date(2026, 9, 23, 9, 6, 0), level: 'info' as const, title: '', detail: '3 件を置換しました' },
    ];
    expect(bugReport(sources({ messages }))).toContain(
      '### 直近のメッセージ\n\n- 09:05:07 [エラー] 保存できません：line one line two\n- 09:06:00 [お知らせ] 3 件を置換しました',
    );
  });

  it('includes the antenna only when asked, as JSON the app opens', () => {
    const project = newProject();
    const text = bugReport(sources({ project }));
    const json = text.match(/```json\n([\s\S]*?)\n```/)?.[1];
    expect(JSON.parse(json!) as Project).toEqual(project);
    expect(bugReport(sources({ includeAntenna: false }))).not.toContain('```json');
  });

  it('says none for empty sections', () => {
    const text = bugReport(sources());
    expect(text).toContain('### 計算結果\n\nまだ計算していません');
    expect(text).toContain('### 警告\n\nなし');
    expect(text).toContain('### 直近のメッセージ\n\nなし');
  });

  it('puts each innermost object or array of the antenna on one line', () => {
    const value = { wires: [{ start: { x: 0, y: -5.3, z: 0 }, tags: ['a, b', '{c}'] }], empty: {}, none: [] };
    expect(compactJson(value)).toBe(
      [
        '{',
        '  "wires": [',
        '    {',
        '      "start": { "x": 0, "y": -5.3, "z": 0 },',
        '      "tags": [ "a, b", "{c}" ]',
        '    }',
        '  ],',
        '  "empty": {},',
        '  "none": []',
        '}',
      ].join('\n'),
    );
  });
});
