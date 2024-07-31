import assert from 'assert'
import { baz as baz2 } from './names.js'
export const { foo: { bar: { baz = '' } = {} } = {} } = { foo: { bar: { baz: 'baz' } } }
assert.strictEqual(baz, 'baz')
assert.strictEqual(baz2, 'baz2')

export { baz, baz2 }
