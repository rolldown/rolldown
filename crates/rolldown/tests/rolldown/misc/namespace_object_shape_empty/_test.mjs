import assert from 'node:assert/strict';
import * as path from 'node:path';
import { barrel, empty } from './dist/main.js';

// `generatedCode.symbols: false` removes `Symbol.toStringTag`, so these namespace objects have no
// own symbol keys. A native namespace object always has `Symbol.toStringTag`.

// `empty.js` has no exports, and no `__reExport` writes into its namespace object. The object has a
// null prototype, and it accepts no new properties.
assert.equal(Object.getPrototypeOf(empty), null);
assert.deepEqual(Reflect.ownKeys(empty), []);
assert.equal(Object.isExtensible(empty), false);
assert.equal(Reflect.set(empty, 'extra', 9), false);

// `barrel.js` has only `export * from 'node:path'`, so `__reExport` fills its namespace object from
// this external module. The object has a null prototype, and it has the keys of `node:path` except
// `default`.
assert.equal(Object.getPrototypeOf(barrel), null);
assert.deepEqual(
  Object.keys(barrel).sort(),
  Object.keys(path)
    .filter((key) => key !== 'default')
    .sort(),
);
assert.equal(barrel.sep, path.sep);
