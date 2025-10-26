import { test, expect, vi } from 'vitest'
import { rolldown } from 'rolldown'

test('validate input option', async () => {
  const consoleSpy = vi.spyOn(console, 'warn')
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
  expect(consoleSpy).toHaveBeenCalledWith(`\x1b[33mWarning: Invalid input options (4 issues found)
- For the "input". Invalid type: Expected (string | Array | Object) but received 1. 
- For the "resolve.foo". Invalid key: Expected never but received "foo". 
- For the "watch.chokidar". The "watch.chokidar" option is deprecated, please use "watch.notify" instead of it. 
- For the "foo". Invalid key: Expected never but received "foo". \x1b[0m`)
})

test('validate output option', async () => {
  const consoleSpy = vi.spyOn(console, 'warn')
  const bundle = await rolldown({
    input: './build-api/main.js',
    cwd: import.meta.dirname,
  })
  await bundle.write({
    // @ts-ignore  invalid key
    foo: 'bar',
    hoistTransitiveImports: false
  })
  expect(consoleSpy).toHaveBeenCalledWith(`\x1b[33mWarning: Invalid output options (1 issue found)
- For the "foo". Invalid key: Expected never but received "foo". \x1b[0m`)
})

test('give a warning for hoistTransitiveImports: true', async () => {
  const consoleSpy = vi.spyOn(console, 'warn')
    const bundle = await rolldown({
      input: './build-api/main.js',
      cwd: import.meta.dirname,
    })
    await bundle.write({
      // @ts-ignore  invalid value
      hoistTransitiveImports: true
    })
    expect(consoleSpy).toHaveBeenCalledWith(`\x1b[33mWarning: Invalid output options (1 issue found)
- For the "hoistTransitiveImports". Invalid type: Expected false but received true. \x1b[0m`)
})
