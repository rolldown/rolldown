import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    external: ['react'],
    jsx: 'react',
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('React.createElement')).toBe(true)
  },
})
