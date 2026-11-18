import assert from 'node:assert';
import { existsSync } from 'node:fs';
import {
  loadBarrel,
  loadDynamic,
  loadExternal,
  loadGetter,
  loadNamespace,
  segNamespace,
} from './dist/index.js';
import * as segBarrel from './dist/seg_barrel.js';
import * as segDynamic from './dist/seg_dynamic.js';
import * as segExternal from './dist/seg_external.js';
import * as segNamespaceFacade from './dist/seg_namespace.js';
import { s_foreign } from './dist/seg_static.js';

// Code outside the bundle loads the facade of each entry (issue #11767).
assert.strictEqual(s_foreign(), 'foreign');
assert.strictEqual(segDynamic.s_foreign(), 'foreign');
assert.strictEqual(segBarrel.s_foreign(), 'foreign');
assert.strictEqual(segBarrel.own, 1);
assert.strictEqual(segExternal.s_foreign(), 'foreign');
assert.strictEqual(typeof segExternal.join, 'function');
assert.strictEqual(segNamespaceFacade.s_foreign(), 'foreign');
// The facade of `seg_getter.js` reads `boom` when it loads, so the test does not load it.
assert.ok(existsSync(new URL('./dist/seg_getter.js', import.meta.url)));

// An `import()` in the bundle reads the namespace object of the entry from the `group` chunk.
const dynamic = await loadDynamic();
assert.strictEqual(dynamic.s_foreign(), 'foreign');
assert.strictEqual(dynamic.x, 37);

const barrel = await loadBarrel();
assert.strictEqual(barrel.s_foreign(), 'foreign');
assert.strictEqual(barrel.x, 37);
assert.strictEqual(barrel.own, 1);

const external = await loadExternal();
assert.strictEqual(external.s_foreign(), 'foreign');

// The getter of `boom` throws. `import()` still resolves, as it does in Node.
const getter = await loadGetter();
assert.strictEqual(getter.s_foreign(), 'foreign');

assert.strictEqual(await loadNamespace(), segNamespace);
assert.strictEqual(segNamespace.s_foreign(), 'foreign');
