// cSpell:disable
import { defineTest } from 'rolldown-tests'
import { getOutputAsset } from 'rolldown-tests/utils'
import { expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

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
        },
        generateBundle() {
          expect(this.getFileName(referenceId)).toMatchInlineSnapshot(
            `"+emitted-umwR9Fta.txt"`,
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
            `"+emitted-umwR9Fta.txt"`,
          )
          expect(asset.originalFileName).toBe(ORIGINAL_FILE_NAME)
          expect(asset.originalFileNames).toStrictEqual([ORIGINAL_FILE_NAME])
          break

        case 'icon.png':
          expect(asset.fileName).toMatchInlineSnapshot(`"icon-eUkSwvpV.png"`)
          break

        default:
          break
      }
    }
  },
})
