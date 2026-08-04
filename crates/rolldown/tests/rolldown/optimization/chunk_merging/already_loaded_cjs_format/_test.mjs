import assert from 'node:assert';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { readdirSync } from 'node:fs';

const require = createRequire(import.meta.url);
const logs = [];
const originalLog = console.log;
console.log = (...args) => logs.push(args.map(String).join(' '));

let keys;
try {
  const main = require(join(import.meta.dirname, 'dist', 'main.js'));
  keys = await main.done;
} finally {
  console.log = originalLog;
}

assert.deepEqual(logs, ['3', '7']);

// The extracted namespace must expose exactly `app.js`'s own exports, even though
// `vendor.js` was merged into the same chunk.
assert.deepEqual(keys, ['done']);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((f) => f.endsWith('.js'));

// main + app + lazy + vendor: `vendor.js` is not inlined into the dynamic
// entry chunk today.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
