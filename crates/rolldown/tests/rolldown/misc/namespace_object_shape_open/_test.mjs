import assert from 'node:assert/strict';
import * as path from 'node:path';
import { commonjsStar, esmStar, externalStar } from './dist/main.js';

// A later `__reExport` adds properties to each of these namespace objects, so each object stays
// open (it accepts new properties). The bundle must load with no `TypeError`. Each object must have
// a null prototype.
for (const namespace of [externalStar, commonjsStar, esmStar]) {
  assert.equal(Object.getPrototypeOf(namespace), null);
  assert.equal(namespace[Symbol.toStringTag], 'Module');
}

// `external-star.js` has `export * from` an external module.
assert.equal(externalStar.fromExternalStar, 'external star');
assert.equal(externalStar.sep, path.sep);

// `commonjs-star.js` has `export * from` a CommonJS module.
assert.equal(commonjsStar.fromCommonjsStar, 'commonjs star');
assert.equal(commonjsStar.fromCommonjs, 'commonjs');

// `esm-star.js` has `export * from` an ES module with dynamic exports.
assert.equal(esmStar.fromEsmStar, 'esm star');
assert.equal(esmStar.fromExternalStar, 'external star');
assert.equal(esmStar.sep, path.sep);
