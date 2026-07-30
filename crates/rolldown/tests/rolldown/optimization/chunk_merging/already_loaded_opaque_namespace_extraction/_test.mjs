import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.map(String).join(' '));
};

let observedKeys;
try {
  const main = await import('./dist/main.js');
  observedKeys = await main.done;
} finally {
  console.log = originalLog;
}

assert.deepEqual(logs, ['3', '7']);

// `main.js` observes the whole namespace of `import('./app.js')`, so it must see
// exactly the entry's own exports — never a generated export name for `vendor.js`,
// which lives in the same chunk after the already-loaded merge.
assert.deepEqual(observedKeys, ['done']);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// https://github.com/rolldown/rolldown/issues/10263
// The opaque namespace use routes through namespace extraction instead of keeping
// `vendor.js` in its own chunk: main + app(+vendor) + lazy.
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
