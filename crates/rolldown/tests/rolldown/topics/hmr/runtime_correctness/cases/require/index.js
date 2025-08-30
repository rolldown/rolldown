import assert from 'node:assert'

const requiredAssignExports = require('./cjs-lib-assign-exports')
assert.strictEqual(requiredAssignExports, 'exports')

const requiredCjsLibReassignModuleExports = require('./cjs-lib-reassign-module-exports')
assert.strictEqual(requiredCjsLibReassignModuleExports(), 'exports')
assert.strictEqual(requiredCjsLibReassignModuleExports.foo, 'foo')

const requireCjsLib = require('./cjs-lib')

assert.strictEqual(requireCjsLib.foo, 'foo')
assert.strictEqual(requireCjsLib.bar, 'bar')
assert.strictEqual(requireCjsLib.baz, undefined)
assert.strictEqual(requireCjsLib.qux, 'qux')

const requiredUmdLib = require('./umd-lib')
assert.strictEqual(requiredUmdLib(), 'exports')
assert.strictEqual(requiredUmdLib.foo, 'foo')

{
  const require = () => 1
  assert.strictEqual(require(), 1)
}

import 'trigger-dep'
