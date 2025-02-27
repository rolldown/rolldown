// @ts-nocheck
import assert from 'node:assert'
import { m1, m2, m3 } from './dist/main'

assert.strictEqual(m1['./dir/a.js'].default, 'a')
assert.strictEqual(m2['/src/dir/a.js'].default, 'a')

assert.strictEqual(m3['./dir/a.js'].default, 'a')
