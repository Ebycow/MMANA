// Client side of engine.worker.ts: one request at a time per worker, answered
// in order. Errors are rejected as problem objects ({ type, title, detail }).

export type EngineRequest = { id: number } & (
  | { kind: 'call'; op: string; request: string }
  | { kind: 'parseMaa'; bytes: Uint8Array }
  | { kind: 'exportMaa'; project: string }
  | { kind: 'optimize'; request: string; cancelUrl: string }
);

export type EngineResponse = { id: number } & (
  | { result: string }
  | { bytes: Uint8Array }
  | { problem: string }
  | { fatal: string }
  | { progress: string }
  | { cancellable: boolean }
);

type Request = EngineRequest extends infer R ? (R extends unknown ? Omit<R, 'id'> : never) : never;

interface Pending {
  resolve(r: EngineResponse): void;
  reject(e: unknown): void;
  onEvent?(r: EngineResponse): void;
}

export class EngineWorker {
  private worker: Worker | null = null;
  private pending = new Map<number, Pending>();
  private nextId = 0;

  private start(): Worker {
    const w = new Worker(new URL('./engine.worker.ts', import.meta.url), { type: 'module' });
    w.onmessage = (ev: MessageEvent<EngineResponse>) => {
      const r = ev.data;
      const p = this.pending.get(r.id);
      if (!p) return;
      if ('progress' in r || 'cancellable' in r) return p.onEvent?.(r);
      this.pending.delete(r.id);
      if ('problem' in r) p.reject(JSON.parse(r.problem));
      else if ('fatal' in r) {
        this.stop({ type: 'internal', title: 'The engine stopped', detail: r.fatal });
        p.reject({ type: 'internal', title: 'The engine stopped', detail: r.fatal });
      } else p.resolve(r);
    };
    w.onerror = (ev) => {
      ev.preventDefault();
      const detail = ev.message || '計算エンジン（wasm）を読み込めません';
      this.stop({ type: 'internal', title: 'The engine stopped', detail });
    };
    return w;
  }

  /** Terminates the worker, rejecting what is pending; the next request starts a new one. */
  stop(problem: { type: string; title: string; detail: string }) {
    this.worker?.terminate();
    this.worker = null;
    for (const p of this.pending.values()) p.reject(problem);
    this.pending.clear();
  }

  send(req: Request, onEvent?: (r: EngineResponse) => void): Promise<EngineResponse> {
    this.worker ??= this.start();
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, onEvent });
      this.worker!.postMessage({ ...req, id });
    });
  }
}
