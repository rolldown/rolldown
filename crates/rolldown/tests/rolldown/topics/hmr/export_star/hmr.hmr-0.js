
import { foo, bar, named } from './sub/index.js'

const { assert } = globalThis.__node;

assert.strictEqual(foo, 'foo')
assert.strictEqual(bar, 'bar')
assert.strictEqual(named, 'named')


import.meta.hot.accept()