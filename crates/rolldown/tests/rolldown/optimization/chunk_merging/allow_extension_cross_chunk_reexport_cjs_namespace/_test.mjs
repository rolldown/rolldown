import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const index = require('./dist/index.js');

assert.strictEqual(index.x, 37);
assert.strictEqual(index.y, 'y');
assert.strictEqual(index.z, 41);
assert.strictEqual(index.keep, 1);

// The emitted entries get no facade (see `mod.rs`).
const files = readdirSync(new URL('./dist', import.meta.url));
assert.deepStrictEqual(
  files.filter((file) => file.startsWith('seg')),
  [],
);

// `import()` reads the namespace object of the entry from the `group` chunk. A write to the
// namespace object fails, as for a module namespace.
const named = await index.load();
assert.strictEqual(named.x, 37);
assert.strictEqual(Reflect.set(named, 'foreign', null), false);
assert.strictEqual((await index.load()).foreign(), 'foreign');

const star = await index.loadStar();
assert.strictEqual(star.s_foreign(), 'foreign');
assert.strictEqual(star.z, 41);
