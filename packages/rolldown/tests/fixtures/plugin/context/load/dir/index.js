import assert from "node:assert";
import * as cjs from './cjs.cjs'

// If this file is marked as `package.json#type: "module"` by rolldown,
// then `cjs.default` should point to `module.exports` of `cjs.cjs`.
assert.deepStrictEqual(cjs.default, { default: 'default' });
