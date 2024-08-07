import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      entryFileNames: '[name]-test.js',
      chunkFileNames: '[name]-chunk.js',
    },
  },
  afterTest: (output) => {
    expect(
      output.output.find((chunk) => (chunk as RolldownOutputChunk).isEntry)
        ?.fileName,
    ).toBe('main-test.js')
    expect(
      output.output.find((chunk) => !(chunk as RolldownOutputChunk).isEntry)
        ?.fileName,
    ).toBe('test-chunk.js')
  },
})
