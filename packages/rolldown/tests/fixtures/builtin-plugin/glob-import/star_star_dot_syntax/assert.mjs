// @ts-nocheck
import assert from 'node:assert'
import { m1 } from './dist/main'

let m = m1['./dir/a.js']
assert.strictEqual(m.default, 'a')
