// cSpell:disable
import { defineTest } from '@tests'
import { expect } from 'vitest'
import { getOutputChunk } from '@tests/utils'
import path from 'node:path'

const renderChunks: any[] = []

export default defineTest({
  config: {
    input: ['main.js', 'entry.js'],
    output: {
      entryFileNames: '[name]-[hash].js',
      chunkFileNames: '[name]-[hash].js',
    },
    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (code, chunk) => {
          renderChunks.push(chunk)
        },
      },
    ],
  },
  afterTest: (output) => {
    // The `RenderChunk` should has file names hash placeholder.
    for (const chunk of renderChunks) {
      switch (chunk.facadeModuleId) {
        case path.join(__dirname, 'main.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"main-!~{000}~.js"`)
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-!~{002}~.js"`)
          expect(chunk.dynamicImports[0]).toMatchInlineSnapshot(
            `"dynamic-!~{003}~.js"`,
          )
          break

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"entry-!~{001}~.js"`)
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-!~{002}~.js"`)
          expect(chunk.dynamicImports).toStrictEqual([])
          break

        case path.join(__dirname, 'dynamic.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"dynamic-!~{003}~.js"`)
          break

        default:
          break
      }
    }

    // The `OutputChunk` file names hash placeholder should be replaced.
    const chunks = getOutputChunk(output)
    for (const chunk of chunks) {
      switch (chunk.facadeModuleId) {
        case path.join(__dirname, 'main.js'):
          expect(chunk.preliminaryFileName).toMatchInlineSnapshot(
            `"main-!~{000}~.js"`,
          )
          expect(chunk.fileName).toMatchInlineSnapshot(`"main-b4IqQb8v.js"`)
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-HPOF0b0V.js"`)
          expect(chunk.dynamicImports[0]).toMatchInlineSnapshot(
            `"dynamic-GH3GEpHx.js"`,
          )
          break

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"entry-4V01jUNm.js"`)
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-HPOF0b0V.js"`)
          expect(chunk.dynamicImports).toStrictEqual([])
          break

        case path.join(__dirname, 'dynamic.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"dynamic-GH3GEpHx.js"`)
          break

        default:
          break
      }
    }
  },
})
