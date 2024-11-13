import { defineTest } from '@tests'
import { assert, expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'plugin-error',
        async resolveId() {
          await errorFn1()
        },
      },
    ],
  },
  catchError(e) {
    assert(e instanceof Error)
    expect(e).toMatchObject({
      message: 'hi',
      extraProp: 1234,
    })
    expect(e.stack).toContain('at errorFn2')
    expect(e.stack).toContain('at errorFn1')
    expect(e.stack).toContain('at PluginContext.resolveId')
  },
})

async function errorFn1() {
  await Promise.resolve()
  await errorFn2()
}

async function errorFn2() {
  await Promise.resolve()
  throw Object.assign(new Error('hi'), { extraProp: 1234 })
}
