import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');

// The chunk's engine namespace carries only the statically-known exports, so
// no callable `then` is observable and `import('./app.js')` settles with the
// namespace itself. A `.then((n) => n.<ns>)` extraction would instead hand
// back the entry's namespace *object*, onto which `__reExport` copies the
// CommonJS `then` at runtime — assimilating the promise into 'intercepted'.
const ns = await app;
assert.notStrictEqual(ns, 'intercepted');
assert.strictEqual(ns.own, 43);
assert.strictEqual(await ns.lazyLoaded, 84);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// Extraction is refused for the `has_dynamic_exports` entry, so `shared.js`
// must stay in its own chunk: main + app(+thenable) + lazy + shared.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
