// @ts-nocheck
import assert from 'node:assert'
import { staticName } from './dist/main'

assert.strictEqual(staticName, "MyClass")
