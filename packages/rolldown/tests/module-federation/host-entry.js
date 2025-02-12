import {
  default as exposeDefault,
  value,
  exposeShared,
  exposeSharedCjs,
} from 'app/expose'
import { foo } from 'app/expose-foo'
import { shared } from 'test-shared'
import { sharedCjs } from 'test-shared-cjs'
import assert from 'node:assert'

assert.strictEqual(value, 'expose')
assert.strictEqual(shared, 'shared')
assert.strictEqual(exposeShared, 'shared')
assert.strictEqual(foo, 'expose-cjs')
assert.strictEqual(sharedCjs, 'shared-cjs')
assert.strictEqual(exposeSharedCjs, 'shared-cjs')
assert.strictEqual(exposeDefault, 'expose-default')

await import('./host-chunk') // create a chunk to make the shared modules to chunk.
