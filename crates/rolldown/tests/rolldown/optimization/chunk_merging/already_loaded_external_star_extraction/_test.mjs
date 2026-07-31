import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.map(String).join(' '));
};

let appNs;
try {
  const main = await import('./dist/main.js');
  appNs = await main.app;
  await appNs.done;
} finally {
  console.log = originalLog;
}

assert.deepEqual(logs, ['3']);

// The runtime `__reExport` merge must forward the external star's names — only
// known at runtime — through the extracted simulated namespace.
assert.strictEqual(typeof appNs.sep, 'string');
assert.strictEqual(await appNs.done, 7);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// https://github.com/rolldown/rolldown/issues/10263
// The external star no longer blocks the already-loaded inlining:
// main + app(+vendor) + lazy.
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
