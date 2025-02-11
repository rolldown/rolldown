import { manifestPlugin } from 'rolldown/experimental'
import { defineTest } from '../../../src/index'
import path from 'node:path'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      assetFileNames: '[name][extname]',
      chunkFileNames: '[name].js',
    },
    plugins: [
      manifestPlugin({
        root: path.resolve(import.meta.dirname),
        outPath: path.resolve(import.meta.dirname, 'dist/manifest.json'),
      }),
      {
        name: 'test',
        buildStart() {
          this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            originalFileName: 'asset.txt',
            source: 'test',
          })
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const manifest = await import('./dist/manifest.json')
    expect(manifest.default).toMatchSnapshot()
  },
})
