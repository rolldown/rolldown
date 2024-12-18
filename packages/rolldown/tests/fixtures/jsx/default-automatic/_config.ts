import { defineTest } from '@tests'
import { expect } from 'vitest'
import { getOutputChunk } from '@tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    external: ['react/jsx-runtime'],
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('react/jsx-runtime')).toBe(true)
  },
})
