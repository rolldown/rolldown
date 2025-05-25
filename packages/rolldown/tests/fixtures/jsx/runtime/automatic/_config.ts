import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    transform: {
      jsx: {
        runtime: 'automatic',
      }
    },
    external: ['react/jsx-runtime'],
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('react/jsx-runtime')).toBe(true)
  },
})
