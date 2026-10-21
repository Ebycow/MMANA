// The three panels of the window: the antenna editor on the left, the 3D view
// above the results on the right. The dividers between them can be dragged,
// and the sizes are remembered per viewer. A panel is hidden by collapsing it,
// from the header's 表示 menu or by dragging it shut.
import { lazy, Suspense, useEffect, useRef } from 'react';
import {
  Group,
  Panel as Pane,
  Separator,
  useDefaultLayout,
  type Layout,
  type LayoutChangedMeta,
  type LayoutStorage,
  type PanelImperativeHandle,
} from 'react-resizable-panels';
import { useLayout, type Shown } from '../state/layout';
import { useApp } from '../state/store';
import { EditorPanel } from './EditorPanel';
import { Panel } from './Panel';
import { PanelBoundary } from './PanelBoundary';
import { ResultsPanel } from './ResultsPanel';

// three.js is the largest dependency; load the 3D view in its own chunk
const AntennaView = lazy(() => import('./AntennaView').then((m) => ({ default: m.AntennaView })));

// sizes are a convenience; without storage the default layout applies
const storage: LayoutStorage = {
  getItem: (key) => {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  },
  setItem: (key, value) => {
    try {
      localStorage.setItem(key, value);
    } catch {
      // not remembered
    }
  },
};

export function Workspace() {
  const fileName = useApp((st) => st.fileName);
  const result = useApp((st) => st.result);
  const shown = useLayout((st) => st.shown);
  const columns = useDefaultLayout({ id: 'mmana.workspace', storage });
  const rows = useDefaultLayout({ id: 'mmana.workspace.right', storage });
  const editor = useRef<PanelImperativeHandle>(null);
  const right = useRef<PanelImperativeHandle>(null);
  const view = useRef<PanelImperativeHandle>(null);
  const results = useRef<PanelImperativeHandle>(null);

  // the view and the results are hidden together by collapsing the right column
  const open = (): Shown | null => {
    if (!editor.current || !right.current || !view.current || !results.current) return null;
    const rightOpen = !right.current.isCollapsed();
    return { editor: !editor.current.isCollapsed(), view: rightOpen && !view.current.isCollapsed(), results: rightOpen && !results.current.isCollapsed() };
  };
  const syncOpen = () => {
    const o = open();
    if (o) useLayout.getState().sync(o);
  };
  const changed = (save: (layout: Layout, meta: LayoutChangedMeta) => void) => (layout: Layout, meta: LayoutChangedMeta) => {
    save(layout, meta);
    if (meta.isUserInteraction) syncOpen();
  };

  // take over the remembered layout, then follow the header's toggles
  useEffect(() => {
    syncOpen();
    const toggle = (pane: PanelImperativeHandle | null, on: boolean) => {
      if (pane && on && pane.isCollapsed()) pane.expand();
      if (pane && !on && !pane.isCollapsed()) pane.collapse();
    };
    return useLayout.subscribe(({ shown: s }, prev) => {
      if (s === prev.shown) return;
      const rightOpen = s.view || s.results;
      // open before shutting, so neither group is left with every panel shut
      if (rightOpen) toggle(right.current, true);
      if (s.editor) toggle(editor.current, true);
      if (s.view) toggle(view.current, true);
      if (s.results) toggle(results.current, true);
      if (rightOpen) {
        toggle(view.current, s.view);
        toggle(results.current, s.results);
      }
      toggle(editor.current, s.editor);
      if (!rightOpen) toggle(right.current, false);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <Group className="workspace" orientation="horizontal" defaultLayout={columns.defaultLayout} onLayoutChanged={changed(columns.onLayoutChanged)}>
      <Pane id="editor" panelRef={editor} className="workspace-pane" defaultSize="44%" minSize="400px" collapsible inert={!shown.editor}>
        <PanelBoundary title="アンテナ" resetKey={fileName}>
          <EditorPanel />
        </PanelBoundary>
      </Pane>
      <Separator className="workspace-separator" />
      <Pane id="right" panelRef={right} defaultSize="56%" minSize="320px" collapsible>
        <Group className="workspace-column" orientation="vertical" defaultLayout={rows.defaultLayout} onLayoutChanged={changed(rows.onLayoutChanged)}>
          <Pane id="view" panelRef={view} className="workspace-pane" defaultSize="38%" minSize="160px" collapsible inert={!shown.view}>
            <PanelBoundary title="アンテナ形状" resetKey={fileName}>
              <Suspense fallback={<Panel title="アンテナ形状">{null}</Panel>}>
                <AntennaView />
              </Suspense>
            </PanelBoundary>
          </Pane>
          <Separator className="workspace-separator" />
          <Pane id="results" panelRef={results} className="workspace-pane" defaultSize="62%" minSize="200px" collapsible inert={!shown.results}>
            <PanelBoundary title="計算結果" resetKey={result}>
              <ResultsPanel />
            </PanelBoundary>
          </Pane>
        </Group>
      </Pane>
    </Group>
  );
}
