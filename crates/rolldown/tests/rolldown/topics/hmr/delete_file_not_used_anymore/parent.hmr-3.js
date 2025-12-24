import { value as childValue } from './child.js'
export { childValue }
export const parentValue = 'parent'

assert.strictEqual(parentValue, 'parent')
assert.strictEqual(childValue, 'child')
