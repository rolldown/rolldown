// @ts-nocheck
import assert from 'node:assert'
import { foo } from './dist/main'

assert.strictEqual(foo, 100)
