import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    jsx: {
      refresh: true,
    },
    external: ['react'],
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('$RefreshReg$')).toBe(true)
  },
})
