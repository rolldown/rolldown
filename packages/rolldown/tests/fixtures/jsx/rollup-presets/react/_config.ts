import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    external: ['react'],
    transform: {
      jsx: {
        runtime: 'classic',
      }
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    // Verify it transforms JSX to React.createElement calls (classic runtime)
    expect(chunk.code.includes('React.createElement')).toBe(true)
  },
})
