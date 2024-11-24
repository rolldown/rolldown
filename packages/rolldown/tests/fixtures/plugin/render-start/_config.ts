import { expect, vi } from 'vitest'
import path from 'node:path'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')
const entryFileNames = '[name]-render-start.js'

const renderStartFn = vi.fn()

export default defineTest({
  skip: true, // FIXME(hyf0): Will be fixed in the next PR
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
          // expect(inputOptions.input).toBe(entry)
          expect(outputOptions.entryFileNames).toBe(entryFileNames)
        },
      },
    ],
  },
  afterTest: () => {
    expect(renderStartFn).toHaveBeenCalledTimes(1)
  },
})
