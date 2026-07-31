import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.map(String).join(' '));
};

let resolved;
try {
  const main = await import('./dist/main.js');
  resolved = await main.app;
} finally {
  console.log = originalLog;
}

assert.deepEqual(logs, ['3']);

// The external star supplies a callable `then` at runtime. The `__reExport`
// merge places it on the extracted namespace, so `import('./app.js')` must
// assimilate through it and settle with the value `then` produces — exactly
// like the unbundled module.
assert.deepEqual(resolved, { hijacked: true });

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// https://github.com/rolldown/rolldown/issues/10263
// The external star no longer blocks the already-loaded inlining:
// main + app(+vendor) + lazy.
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
