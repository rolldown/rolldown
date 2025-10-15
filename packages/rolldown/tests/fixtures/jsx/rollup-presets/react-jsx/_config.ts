import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    external: ['react/jsx-runtime'],
    transform: {
      jsx: {
        runtime: 'automatic',
      }
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    // Verify it uses automatic runtime (imports from jsx-runtime)
    expect(chunk.code.includes('react/jsx-runtime')).toBe(true)
    // Should NOT include React.createElement for automatic runtime
    expect(chunk.code.includes('React.createElement')).toBe(false)
  },
})
