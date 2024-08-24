import assert from 'node:assert'
const { foo } = await import('./lib.js')
const b = (await import('./b.js')).b
import('./a.js').then(({ a }) => {
  assert.strictEqual(a, 1)
})

export { foo, b }
