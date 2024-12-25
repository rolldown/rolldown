import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      entryFileNames: '[name]-[hash:6].js',
      chunkFileNames: '[name]-[hash:7].js',
      cssEntryFileNames: '[name]-[hash:6].css',
      cssChunkFileNames: '[name]-[hash:7].css',
    },
  },
  afterTest: (output) => {
    const hash_entry =
      output.output
        .find((chunk) => (chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-zA-Z0-9_-]+)\.js$/) || []
    const hash_chunk =
      output.output
        .find((chunk) => !(chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-zA-Z0-9_-]+)\.js$/) || []

    const hash_css_entry =
      output.output
        .find(
          (chunk) =>
            chunk.fileName.startsWith('main') && chunk.type === 'asset',
        )
        ?.fileName.match(/-([a-zA-Z0-9_-]+)\.css$/) || []
    const hash_css_chunk =
      output.output
        .find(
          (chunk) =>
            chunk.fileName.startsWith('test') && chunk.type === 'asset',
        )
        ?.fileName.match(/-([a-zA-Z0-9_-]+)\.css$/) || []

    expect(hash_entry[1]).toHaveLength(6)
    expect(hash_chunk[1]).toHaveLength(7)
    expect(hash_css_entry[1]).toHaveLength(6)
    expect(hash_css_chunk[1]).toHaveLength(7)
  },
})
