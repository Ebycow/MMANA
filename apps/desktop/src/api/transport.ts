// One interface for both back ends: Tauri commands in the desktop app, the
// HTTP API (crates/mmana-api) when the UI runs in a browser.
import type {
  CalcResult,
  CalculationRequest,
  FarFieldRequest,
  FarFieldResponse,
  NearFieldRequest,
  NearFieldResponse,
  ParsedProject,
  Project,
  SweepRequest,
  SweepResponse,
} from './types';

/** Error raised by either back end; mirrors the API's problem+json body. */
export class EngineError extends Error {
  constructor(
    readonly type: string,
    readonly title: string,
    readonly detail: string,
  ) {
    super(detail || title);
  }
}

export interface Transport {
  readonly kind: 'tauri' | 'http';
  parseMaa(bytes: Uint8Array): Promise<ParsedProject>;
  calculate(req: CalculationRequest): Promise<CalcResult>;
  sweep(req: SweepRequest): Promise<SweepResponse>;
  farField(req: FarFieldRequest): Promise<FarFieldResponse>;
  nearField(req: NearFieldRequest): Promise<NearFieldResponse>;
}

/** File access differs per platform: native dialogs vs. browser upload/download. */
export interface Files {
  /** Shows an open dialog; resolves to null when cancelled. */
  open(): Promise<{ parsed: ParsedProject; path: string | null; name: string } | null>;
  /**
   * Saves to `path` (or asks for one). Resolves to the path written (null for a
   * browser download), or to null overall when the user cancelled.
   */
  save(project: Project, path: string | null, suggestedName: string): Promise<{ path: string | null } | null>;
}

function toEngineError(e: unknown): EngineError {
  if (e instanceof EngineError) return e;
  if (e && typeof e === 'object' && 'type' in e && 'detail' in e) {
    const p = e as { type: string; title?: string; detail: string };
    return new EngineError(p.type, p.title ?? p.type, p.detail);
  }
  return new EngineError('internal', 'Unexpected error', String(e));
}

export const isTauri = (): boolean => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

// ---------------------------------------------------------------- Tauri ---

async function tauriInvoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toEngineError(e);
  }
}

export const tauriTransport: Transport = {
  kind: 'tauri',
  parseMaa: (bytes) => tauriInvoke('parse_maa', { bytes: Array.from(bytes) }),
  calculate: (request) => tauriInvoke('calculate', { request }),
  sweep: (request) => tauriInvoke('sweep', { request }),
  farField: (request) => tauriInvoke('far_field', { request }),
  nearField: (request) => tauriInvoke('near_field', { request }),
};

const FILTERS = [{ name: 'MMANA アンテナ', extensions: ['maa', 'MAA', 'json'] }];

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** File given on the desktop app's command line, if any. */
export async function startupFile(): Promise<{ parsed: ParsedProject; path: string; name: string } | null> {
  if (!isTauri()) return null;
  return tauriInvoke('startup_file', {});
}

export const tauriFiles: Files = {
  async open() {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({ multiple: false, directory: false, filters: FILTERS });
    if (typeof path !== 'string') return null;
    const parsed = await tauriInvoke<ParsedProject>('open_project', { path });
    return { parsed, path, name: baseName(path) };
  },
  async save(project, path, suggestedName) {
    let target = path;
    if (!target) {
      const { save } = await import('@tauri-apps/plugin-dialog');
      target = await save({ defaultPath: suggestedName, filters: FILTERS });
      if (!target) return null;
    }
    await tauriInvoke('save_project', { path: target, project });
    return { path: target };
  },
};

// ----------------------------------------------------------------- HTTP ---

export function httpTransport(baseUrl: string): Transport {
  const post = async <T>(path: string, body: BodyInit, contentType: string): Promise<T> => {
    let res: Response;
    try {
      res = await fetch(`${baseUrl}${path}`, { method: 'POST', headers: { 'content-type': contentType }, body });
    } catch (e) {
      throw new EngineError('network', 'API に接続できません', `${baseUrl} (${String(e)})`);
    }
    if (!res.ok) throw toEngineError(await res.json().catch(() => ({ type: 'http', detail: res.statusText })));
    return (await res.json()) as T;
  };
  const json = <T>(path: string, body: unknown) => post<T>(path, JSON.stringify(body), 'application/json');
  return {
    kind: 'http',
    parseMaa: (bytes) => post('/v1/maa/parse', new Blob([bytes as BlobPart]), 'application/octet-stream'),
    calculate: (req) => json('/v1/calculations', req),
    sweep: (req) => json('/v1/sweeps', req),
    farField: (req) => json('/v1/far-field', req),
    nearField: (req) => json('/v1/near-field', req),
  };
}

export function browserFiles(transport: Transport, baseUrl: string): Files {
  return {
    open: () =>
      new Promise((resolve, reject) => {
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = '.maa,.MAA,.json';
        input.onchange = async () => {
          const file = input.files?.[0];
          if (!file) return resolve(null);
          try {
            const bytes = new Uint8Array(await file.arrayBuffer());
            const parsed = file.name.toLowerCase().endsWith('.json')
              ? { project: JSON.parse(new TextDecoder().decode(bytes)) as Project, warnings: [] }
              : await transport.parseMaa(bytes);
            resolve({ parsed, path: null, name: file.name });
          } catch (e) {
            reject(toEngineError(e));
          }
        };
        input.click();
      }),
    async save(project, _path, suggestedName) {
      const asJson = suggestedName.toLowerCase().endsWith('.json');
      let blob: Blob;
      if (asJson) {
        blob = new Blob([JSON.stringify(project, null, 2)], { type: 'application/json' });
      } else {
        const res = await fetch(`${baseUrl}/v1/maa/export`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(project),
        });
        if (!res.ok) throw toEngineError(await res.json());
        blob = await res.blob();
      }
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = suggestedName;
      a.click();
      URL.revokeObjectURL(a.href);
      return { path: null };
    },
  };
}

// ------------------------------------------------------------- selection ---

const API_URL = (import.meta.env.VITE_MMANA_API as string | undefined) ?? 'http://127.0.0.1:8080';

export const transport: Transport = isTauri() ? tauriTransport : httpTransport(API_URL);
export const files: Files = isTauri() ? tauriFiles : browserFiles(transport, API_URL);
