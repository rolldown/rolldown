// cSpell:disable
import { defineTest } from '@tests'
import { getOutputAsset } from '@tests/utils'
import { expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

let referenceId: string

export default defineTest({
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
          })
        },
        generateBundle() {
          expect(this.getFileName(referenceId)).toMatchInlineSnapshot(
            `"_emitted-umwR9Fta.txt"`,
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
          expect(asset.fileName).toMatchInlineSnapshot(
            `"_emitted-umwR9Fta.txt"`,
          )
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
