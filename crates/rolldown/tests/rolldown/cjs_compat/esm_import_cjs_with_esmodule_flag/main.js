import assert from 'node:assert';
import * as cjs from './cjs.cjs';
import cjs_default, { default as cjs_default_default, foo as cjs_foo } from './cjs.cjs';
import * as cjs2 from './cjs2.cjs';
import * as cjs3 from './cjs3.cjs';

// In node ESM mode ("type": "module"), __toESM ignores __esModule flag
// and .default represents the whole module.exports, not exports.default.
assert.deepStrictEqual(cjs.default, { default: 'default', foo: 'foo' });
assert.deepStrictEqual(cjs_default, { default: 'default', foo: 'foo' });
assert.deepStrictEqual(cjs_default_default, { default: 'default', foo: 'foo' });

// Cannot inline ns.default.default for `import * as ns`. See bind_imports_and_exports.
assert.deepStrictEqual(cjs.default.default, 'default');
assert.deepStrictEqual(cjs_default_default.default, 'default');

// Inlined via module.exports resolution
assert.deepStrictEqual(cjs.default.foo, 'foo');
assert.deepStrictEqual(cjs.foo, 'foo');
assert.deepStrictEqual(cjs_foo, 'foo');
assert.deepStrictEqual(cjs_default_default.foo, 'foo');

assert.deepStrictEqual(cjs2.default, 'module.exports');

// CJS bailout: cjs3.default is an opaque use of module.exports.
// `bar` is not explicitly imported but must survive tree-shaking.
assert.deepStrictEqual(cjs3.default, { default: 'default', bar: 'bar' });
