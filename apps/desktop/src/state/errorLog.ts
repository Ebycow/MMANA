// The last errors shown or thrown, kept for a bug report: a notification is
// gone by the time the user opens Help.

export interface LoggedError {
  at: Date;
  title: string;
  detail: string;
}

const LIMIT = 20;
const log: LoggedError[] = [];

export function recordError(title: string, detail: string) {
  log.push({ at: new Date(), title, detail });
  if (log.length > LIMIT) log.shift();
}

/** Oldest first. */
export const recentErrors = (): readonly LoggedError[] => [...log];

/** Errors nothing caught, which otherwise only reach the console. */
export function recordUncaughtErrors() {
  window.addEventListener('error', (ev) => recordError('Uncaught error', String(ev.error?.stack ?? ev.message)));
  window.addEventListener('unhandledrejection', (ev) =>
    recordError('Unhandled rejection', String(ev.reason instanceof Error ? (ev.reason.stack ?? ev.reason.message) : ev.reason)),
  );
}
