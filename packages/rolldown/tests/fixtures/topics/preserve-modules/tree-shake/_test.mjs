import lib from './dist/main.js'
import assert from 'node:assert'


assert.strictEqual(lib.lib, 'lib')
assert.strictEqual(lib.a, undefined)

