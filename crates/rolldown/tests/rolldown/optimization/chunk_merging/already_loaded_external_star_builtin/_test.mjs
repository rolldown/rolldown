import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');
const appNs = await app;

assert.strictEqual(typeof appNs.sep, 'string');
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// The external star no longer blocks inlining: main + app(+shared) + lazy.
assert.strictEqual(jsFiles.length, 3, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
