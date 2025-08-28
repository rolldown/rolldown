import assert from 'node:assert'
import { a } from './a'

assert.strictEqual(a.b.c, 'c')
