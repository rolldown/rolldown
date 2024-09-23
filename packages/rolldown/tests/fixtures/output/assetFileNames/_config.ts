import { defineTest } from '@tests'
import { getOutputAssetNames, getOutputFileNames } from '@tests/utils'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      assetFileNames: '[name].[ext]',
    },
    plugins: [
      {
        name: 'test-plugin',
        async buildStart() {
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
    expect(getOutputAssetNames(output)).toStrictEqual([
      'emitted.txt',
      'emitted2.txt',
    ])
  },
})
