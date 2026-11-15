import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// `e.js` also loads `main.js` with `import()`, which runs the entry instead of splitting its chunk, so `d.js` still stays a record and `main.js` needs no facade.
globalThis.events = [];
await import('./dist/main.js');
const e = await globalThis.loadE();
const d = await globalThis.loadD();
await globalThis.loadMain();

assert.deepStrictEqual(globalThis.events, ['d', 'main d']);
assert.strictEqual(e.e, 'e');
assert.strictEqual(d.d, 'd');
const files = fs.readdirSync(path.join(import.meta.dirname, 'dist'));
assert.deepStrictEqual(files.filter((file) => file.endsWith('.js')).sort(), [
  'e.js',
  'main.js',
  'rolldown-runtime.js',
]);
