import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.join(' '));
};

try {
  const main = await import('./dist/main.js');
  // Wait for the dynamic import chain (main -> app -> lazy) to settle.
  await main.done;
} finally {
  console.log = originalLog;
}

assert.deepEqual(logs, ['3', '7']);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// https://github.com/rolldown/rolldown/issues/10263
// `vendor.js` is guaranteed to already be loaded whenever `lazy.js` loads (its
// only dynamic importer, `app.js`, statically imports `vendor.js`), so we should
// get only three chunks:
// - `main.js`
// - `app.js` + `vendor.js`
// - `lazy.js`
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
