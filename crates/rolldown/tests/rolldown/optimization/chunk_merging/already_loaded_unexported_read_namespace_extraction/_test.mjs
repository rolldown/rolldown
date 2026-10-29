import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { app } = await import('./dist/main.js');
const result = await app;

// `n` is not exported by `app.js`, so it must stay `undefined` — even though
// the chunk now also carries `shared.js`'s exports, whose minified names can
// collide with it.
assert.strictEqual(result.n, undefined);
assert.strictEqual(await result.done, 84);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// The merge still applies, through namespace extraction: main + app(+shared) +
// lazy.
assert.strictEqual(jsFiles.length, 3, `Expected 3 chunks but got: ${jsFiles.join(', ')}`);
