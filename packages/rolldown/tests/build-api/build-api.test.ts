import { test, expect } from 'vitest'
import { rolldown } from 'rolldown'
import path from 'node:path'

test('rolldown write twice', async () => {
  const bundle = await rolldown({
    input: './main.js',
    cwd: import.meta.dirname,
  })
  const esmOutput = await bundle.write({
    format: 'esm',
    entryFileNames: 'main.mjs',
  })
  expect(bundle.watchFiles).toStrictEqual([path.join(import.meta.dirname, 'main.js')])
  expect(esmOutput.output[0].fileName).toBe('main.mjs')
  expect(esmOutput.output[0].code).toBeDefined()

  const output = await bundle.write({
    format: 'iife',
    entryFileNames: 'main.js',
  })
  expect(output.output[0].fileName).toBe('main.js')
  expect(output.output[0].code.includes('(function() {')).toBe(true)
})

test('rolldown concurrent write', async () => {
  const bundle = await rolldown({
    input: ['./main.js'],
    cwd: import.meta.dirname,
  })
  await write()
  // Execute twice
  await write()

  async function write() {
    await Promise.all([
      bundle.write({ format: 'esm', dir: './dist' }),
      bundle.write({
        format: 'cjs',
        dir: './dist',
        entryFileNames: 'main.cjs',
      }),
    ])
  }
})

test('should support `Symbol.asyncDispose` of the rolldown bundle and set closed state to true', async () => {
  const bundle = await rolldown({
    input: ['./main.js'],
    cwd: import.meta.dirname,
  })

  await bundle[Symbol.asyncDispose]()
  expect(bundle.closed).toBe(true)
})
