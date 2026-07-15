import assert from 'node:assert/strict';
import value, { __proto__ as proto } from './data.json';

assert.equal(globalThis.jsonStaticImportRan, true);
assert.equal(Object.getPrototypeOf(value), Object.prototype);
assert.equal(Object.prototype.hasOwnProperty.call(value, '__proto__'), true);
assert.deepEqual(proto, { polluted: true });
