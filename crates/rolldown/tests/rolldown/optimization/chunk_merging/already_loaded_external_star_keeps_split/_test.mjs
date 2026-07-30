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

// The external star re-export must stay observable on the dynamic-import namespace.
assert.strictEqual(typeof appNs.sep, 'string');
assert.strictEqual(await appNs.done, 7);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// Namespace extraction is refused for an entry with dynamic exports (external star),
// so `vendor.js` must stay in its own chunk: main + app + lazy + vendor.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
