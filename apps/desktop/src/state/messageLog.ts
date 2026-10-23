// The messages the app showed, kept after their notification is gone: for
// the message list and for a bug report. Errors that only reach the console
// (a panel's drawing error, an uncaught one) are kept too.
import { notificationsStore, type NotificationData } from '@mantine/notifications';
import type { ReactNode } from 'react';
import { create } from 'zustand';

export type MessageLevel = 'error' | 'warning' | 'info';

export interface LoggedMessage {
  at: Date;
  level: MessageLevel;
  title: string;
  detail: string;
}

const LIMIT = 50;

/** Oldest first. */
export const useMessageLog = create<{ messages: LoggedMessage[] }>()(() => ({ messages: [] }));

export function recordMessage(level: MessageLevel, title: string, detail: string) {
  const messages = [...useMessageLog.getState().messages, { at: new Date(), level, title, detail }];
  useMessageLog.setState({ messages: messages.slice(-LIMIT) });
}

export const recordError = (title: string, detail: string) => recordMessage('error', title, detail);

/** The text of a notification's title or message (the app passes strings). */
const text = (node: ReactNode): string => (typeof node === 'string' || typeof node === 'number' ? String(node) : '');

const LEVEL: Record<string, MessageLevel> = { red: 'error', yellow: 'warning' };

/** Keeps every notification shown from now on, and errors nothing caught. */
export function recordMessages() {
  let seen = new Set<string>();
  notificationsStore.subscribe((state) => {
    const all: NotificationData[] = [...state.notifications, ...state.queue];
    for (const n of all) {
      if (n.id && !seen.has(n.id)) recordMessage(LEVEL[n.color ?? ''] ?? 'info', text(n.title), text(n.message));
    }
    seen = new Set(all.flatMap((n) => (n.id ? [n.id] : [])));
  });
  window.addEventListener('error', (ev) => recordError('Uncaught error', String(ev.error?.stack ?? ev.message)));
  window.addEventListener('unhandledrejection', (ev) =>
    recordError('Unhandled rejection', String(ev.reason instanceof Error ? (ev.reason.stack ?? ev.reason.message) : ev.reason)),
  );
}
