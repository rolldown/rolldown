import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: 'main.ts',
    resolve: {
      tsconfig: {
        configFile: 'tsconfig.json'
      },
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes(`should be included`)).toBe(true)
  },
})
