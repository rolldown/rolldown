import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// The lazy `x.js` imports `d.js` and also loads it with `import()`; `d.js` stays a record instead of moving into the chunk of `x.js`, where its `import()` would need a facade file.
globalThis.events = [];
await import('./dist/main.js');
const x = await globalThis.loadX();
const d = await globalThis.loadD();

assert.deepStrictEqual(globalThis.events, ['d']);
assert.strictEqual(x.x, 'xd');
assert.strictEqual(d.d, 'd');
const files = fs.readdirSync(path.join(import.meta.dirname, 'dist'));
assert.deepStrictEqual(files.filter((file) => file.endsWith('.js')).sort(), [
  'main.js',
  'rolldown-runtime.js',
  'x.js',
]);
