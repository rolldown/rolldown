import assert from 'node:assert/strict';
import { esm, ns, star } from './dist/main.js';

// No getter is a bound function. V8 and SpiderMonkey do not cache a read through a bound getter.
// See internal-docs/runtime-helpers/implementation.md
for (const object of [ns, star, esm]) {
  for (const key of Object.getOwnPropertyNames(object)) {
    const { get } = Object.getOwnPropertyDescriptor(object, key);
    if (get) assert.ok(!get.name.startsWith('bound '), key);
  }
}

// `import * as` of a CommonJS module (`__toESM`) and `export *` of it (`__reExport`) read the live value.
assert.equal(ns.value, 1);
assert.equal(star.value, 1);
ns.setValue(2);
assert.equal(ns.value, 2);
assert.equal(star.value, 2);

// The copies use the non-configurable getter of `module.exports`, so a read calls one getter.
const fixed = Object.getOwnPropertyDescriptor(ns.default, 'fixed').get;
assert.equal(ns.fixed, 'fixed');
assert.equal(Object.getOwnPropertyDescriptor(ns, 'fixed').get, fixed);
assert.equal(Object.getOwnPropertyDescriptor(star, 'fixed').get, fixed);

// A configurable getter can change, so the copies read it through a getter of their own.
assert.equal(ns.replaced, 'before');
ns.replace();
assert.equal(ns.replaced, 'after');
assert.equal(star.replaced, 'after');

// `require()` of an ES module (`__toCommonJS`) reads the live value.
assert.equal(esm.__esModule, true);
assert.equal(esm.count, 0);
esm.bump();
assert.equal(esm.count, 1);
