import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { getOutputAssetNames } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    output: {
      assetFileNames: (asset) => {
        expect(asset).toHaveProperty('name')
        expect(asset).toHaveProperty('originalFileName')
        if (
          typeof asset.source === 'string' &&
          asset.source === 'emitted' &&
          asset.type === 'asset'
        ) {
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
          this.emitFile({
            type: 'asset',
            fileName: 'with-name.txt',
            source: 'file-name',
          })
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(getOutputAssetNames(output)).toStrictEqual([
      '1-emitted.txt',
      'foo.txt',
      'with-name.txt',
    ])
  },
})
