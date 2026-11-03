import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');

// `app.js`'s namespace carries the star re-exported callable `then`, so
// `import('./app.js')` must assimilate through it — exactly like the unbundled
// module — and settle with the value that `then` produces. A
// `.then((n) => n.<ns>)` extraction would instead receive that value as `n`
// and read `.<ns>` off it, yielding `undefined`.
assert.strictEqual(await app, 'intercepted');

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// Extraction is refused for the `then`-exposing entry, so `shared.js` must
// stay in its own chunk: main + app(+thenable) + lazy + shared.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
