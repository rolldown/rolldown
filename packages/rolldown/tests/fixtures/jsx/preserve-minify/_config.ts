import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    transform: {
      jsx: 'preserve'
    },
    output: {
      minify: true,
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('<div>Hello World!</div>')).toBe(true)
  },
})
