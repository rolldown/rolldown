import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
const logs = [];
const originalLog = console.log;
console.log = (...a) => logs.push(a.map(String).join(' '));
let keys;
try {
  keys = await (await import('./dist/main.js')).done;
} finally {
  console.log = originalLog;
}
assert.deepEqual(logs, ['3', '7']);
// The alias is what lands in the namespace, so `then` is never a key on it.
assert.deepEqual(keys, ['done', 'ready']);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((f) => f.endsWith('.js'));

// main + app + lazy + vendor: `vendor.js` is not inlined into the dynamic
// entry chunk today.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
