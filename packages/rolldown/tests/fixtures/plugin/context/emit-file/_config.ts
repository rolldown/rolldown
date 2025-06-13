import { defineTest } from 'rolldown-tests'
import { getOutputAsset } from 'rolldown-tests/utils'
import { expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import type { PluginContext } from 'rolldown'

let referenceId: string

const ORIGINAL_FILE_NAME = 'original.txt'

export default defineTest({
  skipComposingJsPlugin: true,
  config: {
    output: {
      assetFileNames: '[name]-[hash].[ext]',
    },
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart() {
          // emit asset string source
          referenceId = this.emitFile({
            type: 'asset',
            name: '+emitted.txt',
            source: 'emitted',
            originalFileName: ORIGINAL_FILE_NAME,
          })
          testEmitFileThis(this.emitFile)
        },
        generateBundle() {
          expect(this.getFileName(referenceId)).toMatchInlineSnapshot(
            `"_emitted-C6bBH0W1.txt"`,
          )
          // emit asset buffer source
          this.emitFile({
            type: 'asset',
            name: 'icon.png',
            source: fs.readFileSync(path.join(__dirname, 'icon.png')),
          })
        },
      },
    ],
  },
  afterTest: (output) => {
    const assets = getOutputAsset(output)
    for (const asset of assets) {
      switch (asset.name) {
        case '+emitted.txt':
          expect(asset.names).toStrictEqual(['+emitted.txt'])
          expect(asset.fileName).toMatchInlineSnapshot(
            `"_emitted-C6bBH0W1.txt"`,
          )
          expect(asset.originalFileName).toBe(ORIGINAL_FILE_NAME)
          expect(asset.originalFileNames).toStrictEqual([ORIGINAL_FILE_NAME])
          break

        case 'icon.png':
          expect(asset.fileName).toMatchInlineSnapshot(`"icon-B5SRLC-l.png"`)
          break

        default:
          break
      }
    }
  },
})

function testEmitFileThis(emitFile: PluginContext['emitFile']) {
  const emitted = emitFile({
    type: 'asset',
    name: 'emitFileThis.txt',
    source: 'emitFileThis',
  })
  expect(emitted).toBeTypeOf('string')
}
