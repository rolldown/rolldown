import { value, exposeShared } from 'app/expose'
import { foo } from 'app/expose-foo'
import { shared } from 'test-shared'
import assert from 'node:assert'
assert.strictEqual(value, 'expose')
assert.strictEqual(shared, 'shared')
assert.strictEqual(exposeShared, 'shared')
assert.strictEqual(foo, 'expose-cjs')

await import('./host-chunk') // create a chunk to make the shared modules to chunk.
