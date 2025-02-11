import { value, exposeShared } from 'app/expose'
import { shared } from 'test-shared'
import assert from 'node:assert'
assert.strictEqual(value, 'expose')
assert.strictEqual(shared, 'shared')
assert.strictEqual(exposeShared, 'shared')
