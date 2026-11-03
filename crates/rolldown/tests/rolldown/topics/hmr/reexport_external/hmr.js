import assert from 'node:assert';
import { assertOk, assertDefault, assertNS, format, value } from './reexport.js';
import { shadowWorks, strictEqual } from './shadow.js';

assert.strictEqual(value, 'from-dep');
assert.strictEqual(typeof assertOk, 'function', 'export { x } from external');
// `default` is a name like any other on the namespace object, but the only named
// re-export shape whose meaning depends on the importee's interop.
assert.strictEqual(assertDefault, assert, 'export { default as y } from external');
assert.strictEqual(typeof assertNS?.ok, 'function', 'export * as ns from external');
assert.strictEqual(typeof format, 'function', 'export * from external');
assert.strictEqual(shadowWorks, true, 'import + export * from the same external');
assert.strictEqual(typeof strictEqual, 'function');

import.meta.hot.accept();
