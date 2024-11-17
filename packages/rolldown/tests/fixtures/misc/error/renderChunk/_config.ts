import { defineTest } from '@tests'
import { assert, expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'my-plugin',
        async renderChunk() {
          await errorFn1()
        },
      },
    ],
  },
  catchError(e) {
    // TODO
    assert(e instanceof Error)
    expect(e).toMatchObject({
      message: 'my-error',
      extraProp: 1234,
    })
    expect(e.stack).toContain('at errorFn2')
    expect(e.stack).toContain('at errorFn1')
    // assert(e instanceof AggregateError)
    // expect(e.message).toContain('my-error')
    // expect(e.message).toContain('at errorFn2')
    // expect(e.message).toContain('at errorFn1')
    // expect(e.errors[0]).toMatchObject({
    //   message: 'my-error',
    //   extraProp: 1234
    // })
  },
})

async function errorFn1() {
  await Promise.resolve()
  await errorFn2()
}

async function errorFn2() {
  await Promise.resolve()
  throw Object.assign(new Error('my-error'), { extraProp: 1234 })
}
