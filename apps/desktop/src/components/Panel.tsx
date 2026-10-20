import { Text } from '@mantine/core';
import type { ReactNode } from 'react';

interface PanelProps {
  title: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  /** Body scrolls by default; pass false for canvases that fill the panel. */
  scroll?: boolean;
}

export function Panel({ title, actions, children, scroll = true }: PanelProps) {
  return (
    <section className="panel">
      <header className="panel-header">
        <Text fw={600} size="sm" style={{ flex: 1, minWidth: 0 }} truncate>
          {title}
        </Text>
        {actions}
      </header>
      <div className="panel-body" style={scroll ? undefined : { overflow: 'hidden', position: 'relative' }}>
        {children}
      </div>
    </section>
  );
}
