import assert from 'node:assert';

import * as cjs from './cjs.cjs';
import cjs_default, { default as cjs_default_default, foo as cjs_foo } from './cjs.cjs';
import * as cjs2 from './cjs2.cjs';
import * as cjs3 from './cjs3.cjs';
import { bar } from './cjs3.cjs';

// If this file is marked as `package.json#type: "module"` by rolldown,
// then `cjs.default` should point to `module.exports` of `cjs.cjs`.
assert.deepStrictEqual(cjs.default, { default: 'default', foo: 'foo' });
assert.deepStrictEqual(cjs_default, { default: 'default', foo: 'foo' });
assert.deepStrictEqual(cjs_default_default, { default: 'default', foo: 'foo' });

// Cannot inline. Check out bind_imports_and_exports for more details.
assert.deepStrictEqual(cjs.default.default, 'default');
assert.deepStrictEqual(cjs_default_default.default, 'default');

assert.deepStrictEqual(cjs.default.foo, 'foo');
assert.deepStrictEqual(cjs.foo, 'foo');
assert.deepStrictEqual(cjs_foo, 'foo');
assert.deepStrictEqual(cjs_default_default.foo, 'foo');

// Doesn't support inline `module.exports`. Not inlined
assert.deepStrictEqual(cjs2.default, 'module.exports');

assert.deepStrictEqual(cjs3.bar(), 'bar');
assert.deepStrictEqual(bar(), 'bar');

// Too deep. Not inlined
assert.deepStrictEqual(cjs3.baz.qux, 'qux');
