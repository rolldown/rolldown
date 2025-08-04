
import assert from 'node:assert';
import { foo, bar, named } from './sub/index.js'


assert.strictEqual(foo, 'foo')
assert.strictEqual(bar, 'bar')
assert.strictEqual(named, 'named')


import.meta.hot.accept()