import assert from 'node:assert'
import { '😈' as devil, moduleFoo } from './dist/main.mjs'

assert.equal(devil, 'devil')
assert.equal((await moduleFoo)['😈'], 'devil')