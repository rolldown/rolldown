import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      entryFileNames: '[name]-[hash:6].js',
      chunkFileNames: '[name]-[hash:7].js',
    },
  },
  afterTest: (output) => {
    const hash_entry =
      output.output
        .find((chunk) => (chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-zA-Z0-9]+)\.js$/) || []
    const hash_chunk =
      output.output
        .find((chunk) => !(chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-zA-Z0-9]+)\.js$/) || []

    expect(hash_entry[1]).toHaveLength(6)
    expect(hash_chunk[1]).toHaveLength(7)
  },
})
