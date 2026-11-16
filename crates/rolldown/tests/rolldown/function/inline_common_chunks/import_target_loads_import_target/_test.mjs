import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// `q.js` loads `d.js` and `d.js` loads `a.js`, both with `import()`; `main` imports `a.js`, which imports `d.js`, so both stay records and `main.js` needs no facade.
globalThis.events = [];
await import('./dist/main.js');
const q = await globalThis.loadQ();
const d = await globalThis.loadD();
const a = await globalThis.loadA();

assert.deepStrictEqual(globalThis.events, ['d', 'main ad']);
assert.strictEqual(q.q, 'q');
assert.strictEqual(d.d, 'd');
assert.strictEqual(a.a, 'ad');
const files = fs.readdirSync(path.join(import.meta.dirname, 'dist'));
assert.deepStrictEqual(files.filter((file) => file.endsWith('.js')).sort(), [
  'main.js',
  'q.js',
  'rolldown-runtime.js',
]);
