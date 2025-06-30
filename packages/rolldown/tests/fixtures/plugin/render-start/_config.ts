import { expect, vi } from 'vitest'
import path from 'node:path'
import { defineTest } from 'rolldown-tests'
import type { NormalizedInputOptions } from 'rolldown'

const entry = path.join(__dirname, './main.js')
const entryFileNames = '[name]-render-start.js'

const renderStartFn = vi.fn()

let buildStartInputOptions: NormalizedInputOptions

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
          expect(inputOptions.input).toStrictEqual([entry])
          expect(outputOptions.entryFileNames).toBe(entryFileNames)
          // ensure same reference
          expect(inputOptions).toBe(buildStartInputOptions)
        },
      },
      {
        name: 'test-plugin-save-build-start-input-options',
        buildStart: (inputOptions) => {
          buildStartInputOptions = inputOptions
        },
      }
    ],
  },
  afterTest: () => {
    expect(renderStartFn).toHaveBeenCalledTimes(1)
  },
})
