import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');
const appNs = await app;

// The indirect star's names must reach the extracted namespace, exactly like the
// unbundled module, where `export * from './barrel.js'` forwards them.
assert.deepEqual(Object.keys(appNs).sort(), ['done', 'marker', 'own']);
assert.strictEqual(appNs.marker, 'from-ext-indirect');
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// The hop does not block inlining: main + app(+shared+barrel) + lazy.
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
