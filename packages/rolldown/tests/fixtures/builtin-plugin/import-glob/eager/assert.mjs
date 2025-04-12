// @ts-nocheck
import assert from 'node:assert'
import { modules1, modules2, modules3, modules4 } from './dist/main'

assert.strictEqual(modules1['./dir/index.js'].value, 1)
assert.strictEqual(modules1['./dir/index.js'].default, 'dir')

assert.strictEqual(modules2['./dir/index.js'], 1)

assert.strictEqual(modules3['./dir/index.js'].value, 1)
assert.strictEqual(modules3['./dir/index.js'].default, 'dir')

assert.strictEqual(modules4['./dir/index.js'], 'dir')
