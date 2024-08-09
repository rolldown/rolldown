import assert from 'node:assert'
import { a } from './dist/main.js'

assert.strictEqual(a, 10000)
