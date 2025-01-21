import { defineTest } from 'rolldown-tests'
import { getOutputAssetNames, getOutputFileNames } from 'rolldown-tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      assetFileNames: (asset) => {
        if (asset.source === 'emitted.txt') {
          return '1-[name].[ext]'
        }
        return '[name].[ext]'
      },
    },
    plugins: [
      {
        name: 'test-plugin',
        async buildStart() {
          this.emitFile({
            type: 'asset',
            name: 'emitted.txt',
            source: 'emitted',
          })
          this.emitFile({
            type: 'asset',
            name: 'foo.txt',
            source: 'foo',
          })
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(getOutputAssetNames(output)).toStrictEqual([
      '1-emitted.txt',
      'foo.txt',
    ])
  },
})
