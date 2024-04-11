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
        renderStart: (outputoptions, inputOptions) => {
          renderStartFn()
          expect(inputOptions.input[0]).toBe(entry)
          expect(outputoptions.entryFileNames).toBe(entryFileNames)
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(renderStartFn).toHaveBeenCalledTimes(1)
  },
})
