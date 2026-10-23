// Compares the wasm engine (web/engine, `npm run build:wasm`) with the native
// one (the `mmana` CLI) on every sample antenna: the browser build must give
// the desktop app's figures. On the golden platform (macOS arm64) they are
// bit-identical, and any difference fails; elsewhere the platform's libm may
// move the last bits (as between macOS, Linux and Windows).
//
//   node scripts/compare-engines.mjs [path/to/mmana]
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const samples = here('../../../testdata/maa/');
const cli = process.argv[2] ?? here('../../../target/release/mmana');

const engine = await import(here('../web/engine/mmana_wasm.js'));
await engine.default({ module_or_path: readFileSync(here('../web/engine/mmana_wasm_bg.wasm')) });

/** Largest relative difference between two JSON values; Infinity when their shapes differ. */
function diff(a, b, path, worst) {
  if (typeof a === 'number' && typeof b === 'number') {
    const d = a === b ? 0 : Math.abs(a - b) / Math.max(Math.abs(a), Math.abs(b), 1e-300);
    if (d > worst.d) Object.assign(worst, { d, path, a, b });
  } else if (Array.isArray(a) && Array.isArray(b) && a.length === b.length) {
    a.forEach((x, i) => diff(x, b[i], `${path}[${i}]`, worst));
  } else if (a && b && typeof a === 'object' && typeof b === 'object') {
    const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
    for (const k of keys) diff(a[k], b[k], `${path}.${k}`, worst);
  } else if (a !== b) {
    Object.assign(worst, { d: Infinity, path, a, b });
  }
}

let overall = 0;
for (const file of readdirSync(samples).filter((f) => f.toUpperCase().endsWith('.MAA')).sort()) {
  const bytes = readFileSync(samples + file);
  const { project } = JSON.parse(engine.parseMaa(bytes));
  const wasm = JSON.parse(engine.call('calculate', JSON.stringify({ project })));
  const native = JSON.parse(execFileSync(cli, ['calc', '--json', samples + file], { encoding: 'utf8', maxBuffer: 1 << 28 }));
  const worst = { d: 0 };
  diff(native, wasm, '', worst);
  overall = Math.max(overall, worst.d);
  console.log(`${file.padEnd(16)} ${worst.d === 0 ? 'identical' : `${worst.d.toExponential(1)} at ${worst.path} (${worst.a} vs ${worst.b})`}`);
}
console.log(`largest relative difference: ${overall}`);
if (overall !== 0) process.exit(1);
