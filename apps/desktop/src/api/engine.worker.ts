// Runs the wasm engine (crates/mmana-wasm) off the UI thread in the browser
// build. Requests and results cross as JSON text, which the engine reads and
// writes anyway.
import type { EngineRequest, EngineResponse } from './wasmEngine';

/** The bindings wasm-bindgen generates (`npm run build:wasm`). */
interface Engine {
  default(): Promise<unknown>;
  call(op: string, request: string): string;
  parseMaa(bytes: Uint8Array): string;
  exportMaa(project: string): Uint8Array;
  optimize(request: string, progress: (json: string) => boolean): string;
}

// Served next to the page rather than bundled, so the desktop build does not
// need the wasm toolchain.
const engine: Promise<Engine> = import(/* @vite-ignore */ `${import.meta.env.BASE_URL}engine/mmana_wasm.js`).then(
  async (m: Engine) => {
    await m.default();
    return m;
  },
);

/** Same pacing as the desktop app: improvements always, the rest every 40 ms. */
const PROGRESS_MS = 40;
/** How often a running optimisation looks for a cancellation. */
const CANCEL_CHECK_MS = 100;

/**
 * The UI cancels an optimisation by revoking a blob URL; a synchronous read
 * of it (allowed in workers) then fails. This needs no cross-origin isolation,
 * which GitHub Pages cannot provide for SharedArrayBuffer.
 */
function revoked(url: string): boolean {
  try {
    const xhr = new XMLHttpRequest();
    xhr.open('GET', url, false);
    xhr.send();
    return false;
  } catch {
    return true;
  }
}

function run(e: Engine, req: EngineRequest, post: (r: EngineResponse) => void): EngineResponse {
  switch (req.kind) {
    case 'call':
      return { id: req.id, result: e.call(req.op, req.request) };
    case 'parseMaa':
      return { id: req.id, result: e.parseMaa(req.bytes) };
    case 'exportMaa':
      return { id: req.id, bytes: e.exportMaa(req.project) };
    case 'optimize': {
      const cancellable = !revoked(req.cancelUrl);
      post({ id: req.id, cancellable });
      let lastPost = -Infinity;
      let lastCheck = performance.now();
      const result = e.optimize(req.request, (json) => {
        const now = performance.now();
        if (now - lastPost >= PROGRESS_MS || (JSON.parse(json) as { improved: boolean }).improved) {
          post({ id: req.id, progress: json });
          lastPost = now;
        }
        if (cancellable && now - lastCheck >= CANCEL_CHECK_MS) {
          lastCheck = now;
          return !revoked(req.cancelUrl);
        }
        return true;
      });
      return { id: req.id, result };
    }
  }
}

self.onmessage = async (ev: MessageEvent<EngineRequest>) => {
  const req = ev.data;
  const post = (r: EngineResponse) => self.postMessage(r);
  try {
    post(run(await engine, req, post));
  } catch (err) {
    // the engine throws a problem as JSON text; anything else is a trap (a
    // panic, out of memory) after which this instance cannot be trusted
    if (typeof err === 'string') post({ id: req.id, problem: err });
    else post({ id: req.id, fatal: String(err) });
  }
};
