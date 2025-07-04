import assert from 'node:assert'

const { foo } = await import('./lib.js')
const a = (await import('./lib.js')).default
const b = (await (() => import('./lib.js'))()).foo
const c = await import('./lib.js').then((c) => c.foo)
import('./lib.js').then(({ foo }) => {
  assert.strictEqual(foo , a)
})

export { foo, a, b, c }
