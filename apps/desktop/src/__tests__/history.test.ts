import { beforeEach, describe, expect, it } from 'vitest';
import type { Project } from '../api/types';
import { describeChange } from '../domain/history';
import { newProject, newWire } from '../domain/project';
import { useApp } from '../state/store';

const changed = (recipe: (p: Project) => void) => {
  const before = newProject();
  const after = structuredClone(before);
  recipe(after);
  return describeChange(before, after);
};

describe('naming a step of the history', () => {
  it('names the wires changed, or how many when there are many', () => {
    expect(changed((p) => void (p.antenna.wires[0].end.z = 1))).toBe('ワイヤー 1 を変更');
    const four = (p: Project) => {
      p.antenna.wires = [0, 1, 2, 3].map((i) => newWire({ x: i, y: 0, z: 0 }, { x: i, y: 1, z: 0 }));
    };
    const before = newProject();
    four(before);
    const after = structuredClone(before);
    after.antenna.wires.forEach((w) => (w.radius = 0.002));
    expect(describeChange(before, after)).toBe('ワイヤー 4 本を変更');
  });

  it('counts wires added and removed', () => {
    expect(changed((p) => void p.antenna.wires.push(newWire({ x: 0, y: 0, z: 0 }, { x: 1, y: 0, z: 0 })))).toBe('ワイヤーを 1 本追加');
    expect(changed((p) => void (p.antenna.wires = []))).toBe('ワイヤーを 1 本削除');
  });

  it('gives the new name and frequency', () => {
    expect(changed((p) => void (p.antenna.name = '3el'))).toBe('アンテナ名を「3el」に変更');
    expect(changed((p) => void (p.antenna.frequencyMhz = 7.05))).toBe('設計周波数を 7.05 MHz に変更');
  });

  it('names the part of the environment', () => {
    expect(changed((p) => void (p.environment.height = 10))).toBe('地上高を変更');
  });

  it('names two changes and shortens more', () => {
    expect(changed((p) => void ((p.antenna.name = 'a'), (p.environment.height = 10)))).toBe('アンテナ名を「a」に変更、地上高を変更');
    expect(changed((p) => void ((p.antenna.name = 'a'), (p.environment.height = 10), (p.antenna.comment = 'c')))).toBe(
      'アンテナ名を「a」に変更、コメントを変更 など',
    );
  });

  it('says when nothing changed', () => {
    expect(changed(() => {})).toBe('変更なし');
  });
});

describe('the history list', () => {
  beforeEach(() => useApp.getState().reset());

  const rename = (name: string) => useApp.getState().edit((p) => void (p.antenna.name = name));
  const labels = () => {
    const { past, future } = useApp.getState();
    return { done: past.map((e) => e.label), undone: future.map((e) => e.label) };
  };

  it('keeps each step its name through undo and redo', () => {
    rename('a');
    useApp.getState().edit((p) => void (p.environment.height = 10), { label: '手で付けた名前' });
    useApp.getState().undo();
    expect(labels()).toEqual({ done: ['アンテナ名を「a」に変更'], undone: ['手で付けた名前'] });
    useApp.getState().redo();
    expect(labels()).toEqual({ done: ['アンテナ名を「a」に変更', '手で付けた名前'], undone: [] });
  });

  it('travels several steps back and forth', () => {
    rename('a');
    rename('b');
    rename('c');
    useApp.getState().travel(-3);
    expect(useApp.getState().project.antenna.name).toBe('新規アンテナ');
    useApp.getState().travel(2);
    expect(useApp.getState().project.antenna.name).toBe('b');
    expect(labels().undone).toEqual(['アンテナ名を「c」に変更']);
  });
});
