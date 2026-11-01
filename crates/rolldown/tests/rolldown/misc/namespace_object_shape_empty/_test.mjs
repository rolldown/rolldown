import assert from 'node:assert/strict';
import * as path from 'node:path';
import { barrel, empty } from './dist/main.js';

// `generatedCode.symbols: false` removes `Symbol.toStringTag`, so these namespace objects have no
// own symbol keys. A native namespace object always has `Symbol.toStringTag`.

// An empty namespace object that no `__reExport` writes into: a null prototype, and no new
// properties.
assert.equal(Object.getPrototypeOf(empty), null);
assert.deepEqual(Reflect.ownKeys(empty), []);
assert.equal(Object.isExtensible(empty), false);
assert.equal(Reflect.set(empty, 'extra', 9), false);

// An empty namespace object that `__reExport` fills from an external module: a null prototype,
// and the keys of the external module without `default`.
assert.equal(Object.getPrototypeOf(barrel), null);
assert.deepEqual(
  Object.keys(barrel).sort(),
  Object.keys(path)
    .filter((key) => key !== 'default')
    .sort(),
);
assert.equal(barrel.sep, path.sep);
