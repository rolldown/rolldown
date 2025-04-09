import { test, expect } from 'vitest'
import { rolldown } from 'rolldown'

test('validate input option', async () => {
  try {
    await rolldown({
      // @ts-ignore invalid value
      input: 1,
      cwd: import.meta.dirname,
      // @ts-ignore invalid key
      foo: 'bar',
      resolve: {
        // @ts-ignore nested invalid key
        foo: 'bar',
      },
      watch: {
        // @ts-ignore
        chokidar: {},
      },
      experimental: {
        hmr: {}
      }
    })
    expect.unreachable()
  } catch (error: any) {
    expect(error.message).toMatchInlineSnapshot(`
      "Failed validate input options.
      - For the "input". Invalid type: Expected (string | Array | Object) but received 1. 
      - For the "resolve.foo". Invalid key: Expected never but received "foo". 
      - For the "watch.chokidar". The "watch.chokidar" option is deprecated, please use "watch.notify" instead of it. 
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
      // @ts-ignore  invalid key
      foo: 'bar',
    })
    expect.unreachable()
  } catch (error: any) {
    expect(error.message).toMatchInlineSnapshot(`
        "Failed validate output options.
        - For the "foo". Invalid key: Expected never but received "foo". "
      `)
  }
})
