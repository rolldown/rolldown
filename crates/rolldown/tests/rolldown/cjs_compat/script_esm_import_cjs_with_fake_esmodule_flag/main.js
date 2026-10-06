import assert from 'node:assert';
import fake from './fake-esmodule.cjs';
import * as ns from './fake-esmodule.cjs';

// This file sits in a package without `"type": "module"`, so its imports get
// Babel-style interop. The importee fakes `__esModule` without an own
// `default`, so `default` must fall back to `module.exports` (#10360).
const { __extends } = fake;
assert.strictEqual(__extends(), 'EXTENDS');
assert.strictEqual(fake.__awaiter(), 'AWAITER');
assert.strictEqual(ns.default, fake);
assert.strictEqual(ns.__extends, __extends);

const dyn = await import('./fake-esmodule.cjs');
assert.strictEqual(dyn.default.__extends, __extends);
