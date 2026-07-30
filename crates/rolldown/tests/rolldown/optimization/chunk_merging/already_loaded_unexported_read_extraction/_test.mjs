import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.map(String).join(' '));
};

try {
  const main = await import('./dist/main.js');
  await main.done;
} finally {
  console.log = originalLog;
}

// `app.missing` must stay `undefined`: the extracted simulated namespace only holds
// the entry's own exports, so no generated export of the merged chunk can shadow it.
assert.deepEqual(logs, ['3', 'undefined', '7']);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// https://github.com/rolldown/rolldown/issues/10263
// The unexported-name read routes through namespace extraction instead of keeping
// `vendor.js` in its own chunk: main + app(+vendor) + lazy.
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
