import { isTauri } from './api/transport';

// filled in by vite.config.ts at build time
declare const __MMANA_VERSION__: string;
declare const __MMANA_BUILT_AT__: string;
declare const __MMANA_BUILD_PLATFORM__: string;

export const VERSION = __MMANA_VERSION__;
export const BUILT_AT = new Date(__MMANA_BUILT_AT__);

/** Where this copy runs: the desktop app's OS, the browser version, or the UI against the HTTP API. */
export const PLATFORM = isTauri() ? __MMANA_BUILD_PLATFORM__ : import.meta.env.MODE === 'web' ? 'Web' : 'HTTP API';

const pad = (n: number) => String(n).padStart(2, '0');

/** Local time without seconds, e.g. 2026-10-23 14:05. */
export function formatBuiltAt(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** One line to paste into a bug report. */
export const versionLine = (): string => `MMANA ${VERSION} (${PLATFORM}, built ${BUILT_AT.toISOString()})`;
