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
          // the asset file name should be deconflict
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
