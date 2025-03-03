import { test, expect } from 'vitest'
import { rolldown } from 'rolldown'

test('validate input option', async () => {
  try {
    await rolldown({
      input: 1, // invalid value
      cwd: import.meta.dirname,
      foo: 'bar', // invalid key
      resolve: {
        foo: 'bar', // nested invalid key
      },
    })
    expect.unreachable()
  } catch (error: any) {
    expect(error.message).toMatchInlineSnapshot(`
          "Failed validate input options.
          - For the "input". Invalid type: Expected (string | Array | Object) but received 1. 
          - For the "resolve.foo". Invalid key: Expected never but received "foo". 
          - For the "foo". Invalid key: Expected never but received "foo". "
        `)
  }
})

test('validate output option', async () => {
  try {
    const bundle = await rolldown({
      input: './build-api/main.js',
      cwd: import.meta.dirname,
    })
    await bundle.write({
      foo: 'bar', // invalid key
    })
    expect.unreachable()
  } catch (error: any) {
    expect(error.message).toMatchInlineSnapshot(`
        "Failed validate output options.
        - For the "foo". Invalid key: Expected never but received "foo". "
      `)
  }
})
