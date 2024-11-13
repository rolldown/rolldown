// cSpell:disable
import { expect, vi } from 'vitest'
import { defineTest } from '@tests'
import { getOutputChunk } from '@tests/utils'
import path from 'node:path'

const fn = vi.fn()

let isComposingJs = false
export default defineTest({
  beforeTest(testKind) {
    isComposingJs = testKind === 'compose-js-plugin'
  },
  config: {
    input: ['main.js', 'entry.js'],
    output: {
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
    },
    plugins: [
      {
        name: 'test-plugin',
        augmentChunkHash: (chunk) => {
          fn()
          expect(Object.values(chunk.modules)[0].code).toBe('console.log();\n')
          expect(Object.values(chunk.modules)[0].renderedLength).toBe(15)
          if (chunk.fileName.includes('entry')) {
            return 'entry-hash'
          }
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(fn).toHaveBeenCalledTimes(2)
    const chunks = getOutputChunk(output)
    for (const chunk of chunks) {
      switch (chunk.facadeModuleId) {
        case path.join(__dirname, 'main.js'):
          isComposingJs
            ? expect(chunk.fileName).toMatchInlineSnapshot(`"main-z7Zg_USA.js"`)
            : expect(chunk.fileName).toMatchInlineSnapshot(`"main-z7Zg_USA.js"`)
          break

        case path.join(__dirname, 'entry.js'):
          isComposingJs
            ? expect(chunk.fileName).toMatchInlineSnapshot(
                `"entry-tnETJw_E.js"`,
              )
            : expect(chunk.fileName).toMatchInlineSnapshot(
                `"entry-tnETJw_E.js"`,
              )
          break

        default:
          break
      }
    }
  },
})
