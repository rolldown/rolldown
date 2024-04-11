import { expect, vi } from 'vitest'
import path from 'node:path'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')
const entryFileNames = '[name]-render-start.js'

const renderStartFn = vi.fn()

export default defineTest({
  config: {
    input: entry,
    output: {
      entryFileNames,
    },
    plugins: [
      {
        name: 'test-plugin-render-start',
        renderStart: (outputOptions, inputOptions) => {
          renderStartFn()
          expect((inputOptions.input as string[])[0]).toBe(entry)
          expect(outputOptions.entryFileNames).toBe(entryFileNames)
        },
      },
    ],
  },
  afterTest: () => {
    expect(renderStartFn).toHaveBeenCalledTimes(1)
  },
})
