import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// `a` carries `m.js`, so its `import()` reads the registry without loading a file.
globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
const ns = await globalThis.loadFromA();

assert.deepStrictEqual(globalThis.events, ['M body', 'A 1', 'B 2']);
assert.strictEqual(ns.count, 2);
assert.strictEqual(ns[Symbol.toStringTag], 'Module');
const files = fs.readdirSync(path.join(import.meta.dirname, 'dist'));
assert.deepStrictEqual(files.filter((file) => file.endsWith('.js')).sort(), [
  'a.js',
  'b.js',
  'rolldown-runtime.js',
]);
