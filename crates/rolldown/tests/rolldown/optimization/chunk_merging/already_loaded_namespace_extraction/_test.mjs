import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');
const appNs = await app;

// The `import()` namespace must expose exactly `app.js`'s own exports even
// though `shared.js` was grouped into the same chunk: its `value` export is
// published for `lazy.js` under a mangled name, but must not leak into the
// namespace observed by the dynamic importer.
assert.deepEqual(Object.keys(appNs).sort(), ['done', 'own']);
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// main + app(+shared) + lazy
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
