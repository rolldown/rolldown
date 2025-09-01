import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.ts',
    tsconfig: 'tsconfig.json',
    transform: {
      typescript: {
        onlyRemoveTypeImports: true,
      },
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes(`should not be removed`)).toBe(true)
  },
})
