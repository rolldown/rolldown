import assert from 'node:assert'
import { bar } from './commonjs.js'
assert.equal(bar, 1)

export * from './commonjs.js';
