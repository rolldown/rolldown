import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');
const appNs = await app;

// The `import()` namespace must include the external star re-export's names
// alongside `app.js`'s own exports, exactly like the unbundled module.
assert.strictEqual(appNs.marker, 'from-ext');
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// main + app + lazy + shared: `shared.js` is not inlined into the dynamic
// entry chunk today.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
