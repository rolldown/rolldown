import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      entryFileNames: '[name]-test.js',
      chunkFileNames: '[name]-chunk.js',
      cssEntryFileNames: '[name]-test.css',
      cssChunkFileNames: '[name]-chunk.css',
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

    expect(
      output.output.find(
        (chunk) => (chunk as RolldownOutputChunk).fileName === 'main-test.css',
      ),
    ).toBeTruthy()
    expect(
      output.output.find(
        (chunk) => (chunk as RolldownOutputChunk).fileName === 'test-chunk.css',
      ),
    ).toBeTruthy()
  },
})
