import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      hashCharacters: 'hex',
      entryFileNames: '[name]-[hash]-[hash:6].js',
      chunkFileNames: '[name]-[hash]-[hash:7].js',
      cssEntryFileNames: '[name]-[hash:6]-[hash:8].css',
      cssChunkFileNames: '[name]-[hash:7]-[hash:9].css',
    },
  },
  afterTest: (output) => {
    const hash_entry =
      output.output
        .find((chunk) => (chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-f0-9]+)-([a-f0-9]+)\.js$/) || []
    const hash_chunk =
      output.output
        .find((chunk) => !(chunk as RolldownOutputChunk).isEntry)
        ?.fileName.match(/-([a-f0-9]+)-([a-f0-9]+)\.js$/) || []

    const hash_css_entry =
      output.output
        .find(
          (chunk) =>
            chunk.fileName.startsWith('main') && chunk.type === 'asset',
        )
        ?.fileName.match(/-([a-f0-9]+)-([a-f0-9]+)\.css$/) || []
    const hash_css_chunk =
      output.output
        .find(
          (chunk) =>
            chunk.fileName.startsWith('test') && chunk.type === 'asset',
        )
        ?.fileName.match(/-([a-f0-9]+)-([a-f0-9]+)\.css$/) || []

    expect(hash_entry[1]).toHaveLength(8)
    expect(hash_entry[2]).toHaveLength(6)
    expect(hash_chunk[1]).toHaveLength(8)
    expect(hash_chunk[2]).toHaveLength(7)
    expect(hash_css_entry[1]).toHaveLength(6)
    expect(hash_css_entry[2]).toHaveLength(8)
    expect(hash_css_chunk[1]).toHaveLength(7)
    expect(hash_css_chunk[2]).toHaveLength(9)
  },
})
