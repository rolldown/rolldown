import { manifestPlugin } from 'rolldown/experimental'
import { defineTest } from '../../../src/index'
import path from 'path'

export default defineTest({
  config: {
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
            source: 'test',
          })
        },
      },
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
