import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { getOutputAssetNames } from 'rolldown-tests/utils'
import type { OutputChunk } from 'rolldown'

export default defineTest({
  config: {
    output: {
      entryFileNames() {
        return '[name]-test.js'
      },
      chunkFileNames() {
        return '[name]-chunk.js'
      },
      cssEntryFileNames() {
        return '[name]-test.css'
      },
      cssChunkFileNames() {
        return '[name]-chunk.css'
      },
      assetFileNames() {
        return '[name]-asset[extname]'
      },
    },
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          // deduplicate assets if an explicit fileName is not provided
          this.emitFile({
            type: 'asset',
            name: 'emitted.txt',
            source: 'emitted',
          })
          this.emitFile({
            type: 'asset',
            name: 'emitted.txt',
            source: 'emitted',
          })
          // the asset file name should be deconflict
          this.emitFile({
            type: 'asset',
            name: 'emitted.txt',
            source: 'foo',
          })
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(
      output.output.find((chunk) => (chunk as OutputChunk).isEntry)?.fileName,
    ).toBe('main-test.js')
    expect(
      output.output.find((chunk) => !(chunk as OutputChunk).isEntry)?.fileName,
    ).toBe('test-chunk.js')

    expect(getOutputAssetNames(output)).toStrictEqual([
      'emitted-asset.txt',
      'emitted-asset2.txt',
      'main-test.css',
      'test-chunk.css',
    ])
  },
})
