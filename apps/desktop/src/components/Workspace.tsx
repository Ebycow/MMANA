// The three panels of the window: the antenna editor on the left, the 3D view
// above the results on the right. The dividers between them can be dragged,
// and the sizes are remembered per viewer.
import { lazy, Suspense } from 'react';
import { Group, Panel as Pane, Separator, useDefaultLayout, type LayoutStorage } from 'react-resizable-panels';
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
  const columns = useDefaultLayout({ id: 'mmana.workspace', storage });
  const rows = useDefaultLayout({ id: 'mmana.workspace.right', storage });

  return (
    <Group className="workspace" orientation="horizontal" defaultLayout={columns.defaultLayout} onLayoutChanged={columns.onLayoutChanged}>
      <Pane id="editor" className="workspace-pane" defaultSize="44%" minSize="400px">
        <PanelBoundary title="アンテナ" resetKey={fileName}>
          <EditorPanel />
        </PanelBoundary>
      </Pane>
      <Separator className="workspace-separator" />
      <Pane id="right" defaultSize="56%" minSize="320px">
        <Group className="workspace-column" orientation="vertical" defaultLayout={rows.defaultLayout} onLayoutChanged={rows.onLayoutChanged}>
          <Pane id="view" className="workspace-pane" defaultSize="38%" minSize="160px">
            <PanelBoundary title="アンテナ形状" resetKey={fileName}>
              <Suspense fallback={<Panel title="アンテナ形状">{null}</Panel>}>
                <AntennaView />
              </Suspense>
            </PanelBoundary>
          </Pane>
          <Separator className="workspace-separator" />
          <Pane id="results" className="workspace-pane" defaultSize="62%" minSize="200px">
            <PanelBoundary title="計算結果" resetKey={result}>
              <ResultsPanel />
            </PanelBoundary>
          </Pane>
        </Group>
      </Pane>
    </Group>
  );
}
