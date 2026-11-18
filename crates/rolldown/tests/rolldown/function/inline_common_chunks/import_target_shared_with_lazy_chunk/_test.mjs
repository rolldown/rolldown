import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// `main` and the lazy `p.js` both import `d.js`, which `main` also loads with `import()`; `d.js` stays a record, so `main.js` needs no facade.
globalThis.events = [];
await import('./dist/main.js');
const d = await globalThis.loadD();
const p = await globalThis.loadP();

assert.deepStrictEqual(globalThis.events, ['d', 'main d']);
assert.strictEqual(d.d, 'd');
assert.strictEqual(p.p, 'pd');
const files = fs.readdirSync(path.join(import.meta.dirname, 'dist'));
assert.deepStrictEqual(files.filter((file) => file.endsWith('.js')).sort(), [
  'main.js',
  'p.js',
  'rolldown-runtime.js',
]);
