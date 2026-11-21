import assert from 'node:assert';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const index = require('./dist/index.js');
const segNamed = require('./dist/seg_named.js');
const segBarrel = require('./dist/seg_barrel.js');
const segStar = require('./dist/seg_star.js');
const segMerged = require('./dist/seg_merged.js');

assert.strictEqual(index.x, 37);
assert.strictEqual(index.y, 'y');
assert.strictEqual(index.z, 41);
assert.strictEqual(index.keep, 1);
assert.strictEqual(segNamed.foreign(), 'foreign');
assert.strictEqual(segNamed.x, 37);
assert.strictEqual(segBarrel.s_foreign(), 'foreign');
assert.strictEqual(segBarrel.s_y, 'y');
assert.strictEqual(segStar.s_foreign(), 'foreign');
assert.strictEqual(segMerged.s_foreign(), 'foreign');
assert.strictEqual(segMerged.s_keep, 1);
assert.strictEqual(segMerged.w, 37);

// `import()` reads the namespace object of the entry from the `group` chunk, not the exports
// object of the facade. A write to the namespace object fails, as for a module namespace.
const named = await index.load();
assert.strictEqual(named.x, 37);
assert.strictEqual(Reflect.set(named, 'foreign', null), false);
assert.strictEqual((await index.load()).foreign(), 'foreign');

const star = await index.loadStar();
assert.strictEqual(star.s_foreign(), 'foreign');
assert.strictEqual(star.z, 41);
