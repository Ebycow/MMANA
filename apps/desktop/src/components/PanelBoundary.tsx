// Keeps an error while drawing one panel inside that panel, so the rest of
// the window (and the unsaved antenna) stays usable.
import { Button, Stack, Text } from '@mantine/core';
import { Component, type ErrorInfo, type ReactNode } from 'react';
import { recordError } from '../state/errorLog';
import { Panel } from './Panel';

interface Props {
  title: string;
  children: ReactNode;
  /** The panel is drawn again when this changes (a new result, another file). */
  resetKey?: unknown;
}

export class PanelBoundary extends Component<Props, { error: Error | null; key: unknown }> {
  state = { error: null as Error | null, key: this.props.resetKey };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  static getDerivedStateFromProps(props: Props, state: { error: Error | null; key: unknown }) {
    return props.resetKey !== state.key ? { error: null, key: props.resetKey } : null;
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
    recordError(`表示エラー（${this.props.title}）`, `${error.stack ?? error.message}${info.componentStack ?? ''}`);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <Panel title={this.props.title}>
        <Stack align="center" gap="xs" p="xl">
          <Text size="sm">この欄の表示中にエラーが起きました。</Text>
          <Text size="xs" c="dimmed" ta="center">
            {String(this.state.error.message || this.state.error)}
          </Text>
          <Button size="xs" variant="default" onClick={() => this.setState({ error: null })}>
            もう一度表示する
          </Button>
        </Stack>
      </Panel>
    );
  }
}
