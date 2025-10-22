import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { manifestPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    output: {
      chunkFileNames: '[name].js',
      assetFileNames: '[name][extname]',
    },
    plugins: [
      manifestPlugin({
        root: path.resolve(import.meta.dirname),
        outPath: path.resolve(import.meta.dirname, 'dist/manifest.json'),
        cssEntries: () => new Map(),
      }),
      {
        name: 'test',
        buildStart() {
          this.emitFile({
            type: 'asset',
            name: 'asset.txt',
            source: 'hello world',
            originalFileName: 'asset.txt',
          })
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const manifest = await import('./dist/manifest.json')
    await expect(manifest.default).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, "manifest.json.snap")
    )
  },
})
