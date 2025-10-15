import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.jsx',
    transform: {
      jsx: 'preserve'
    }
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    // Verify JSX is preserved in the output (not transformed)
    expect(chunk.code.includes('<div>test</div>')).toBe(true)
    // Should not include React.createElement when preserving
    expect(chunk.code.includes('React.createElement')).toBe(false)
  },
})
