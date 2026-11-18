import assert from 'node:assert';
import * as ns from './dep.js';

// Each write targets an object below the namespace member, which is legal.
function setKey(key) {
  ns.obj[key] = 1;
}
setKey('k');
ns.fn().x = 1;

assert.strictEqual(ns.obj.k, 1);
assert.strictEqual(ns.fn().x, 1);
assert.strictEqual(ns.C.poke(), 1);
