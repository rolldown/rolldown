import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');
const appNs = await app;

// The `import()` namespace must include the external star re-export's names
// alongside `app.js`'s own exports, exactly like the unbundled module. A
// `.then((n) => n.<ns>)` extraction would return the synthetic namespace,
// which cannot carry names only known at runtime.
assert.strictEqual(appNs.marker, 'from-ext');
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// Extraction is refused for the entry whose `export *` chain reaches an
// external module, so `shared.js` must stay in its own chunk: main + app +
// lazy + shared.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
