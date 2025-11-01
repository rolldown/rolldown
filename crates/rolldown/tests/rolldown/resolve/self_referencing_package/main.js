import assert from 'node:assert'
import { internal } from 'self-ref-lib'

// Test self-referencing package through exports field
assert.strictEqual(internal, 'internal-value')
