import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'my-plugin',
        async resolveId() {
          await errorFn1()
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e.message).toContain('my-error')
    expect(e.message).toContain('at errorFn2')
    expect(e.message).toContain('at errorFn1')
    expect(e.errors[0]).toMatchObject({
      message: 'my-error',
      extraProp: 1234,
      code: 'PLUGIN_ERROR',
      plugin: 'my-plugin',
      hook: 'resolveId',
    })
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
