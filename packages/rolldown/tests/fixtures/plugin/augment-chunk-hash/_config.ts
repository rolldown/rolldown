import { expect, vi } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { getOutputChunk } from 'rolldown-tests/utils'
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
          if (chunk.fileName.includes('entry')) {
            expect(Object.values(chunk.modules)[0].code).toBe(
              '//#region entry.js\nconsole.log();\n\n//#endregion',
            )
            expect(Object.values(chunk.modules)[0].renderedLength).toBe(47)
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
            ? expect(chunk.fileName).toMatchInlineSnapshot(`"main-BTVONCL2.js"`)
            : expect(chunk.fileName).toMatchInlineSnapshot(`"main-BTVONCL2.js"`)
          break

        case path.join(__dirname, 'entry.js'):
          isComposingJs
            ? expect(chunk.fileName).toMatchInlineSnapshot(
                `"entry-BS2ltxwY.js"`,
              )
            : expect(chunk.fileName).toMatchInlineSnapshot(
                `"entry-BS2ltxwY.js"`,
              )
          break

        default:
          break
      }
    }
  },
})
