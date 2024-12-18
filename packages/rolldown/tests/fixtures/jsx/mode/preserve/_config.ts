import { defineTest } from '@tests'
import { expect } from 'vitest'
import { getOutputChunk } from '@tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    jsx: {
      mode: 'preserve',
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('<div>test</div>')).toBe(true)
  },
})
