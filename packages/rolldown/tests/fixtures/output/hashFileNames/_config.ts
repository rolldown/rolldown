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
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-!~{003}~.js"`)
          expect(chunk.dynamicImports[0]).toMatchInlineSnapshot(
            `"dynamic-!~{002}~.js"`,
          )
          break

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"entry-!~{001}~.js"`)
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-!~{003}~.js"`)
          expect(chunk.dynamicImports).toStrictEqual([])
          break

        case path.join(__dirname, 'dynamic.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"dynamic-!~{002}~.js"`)
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
          expect(chunk.fileName).toMatchInlineSnapshot(`"main-zpPZMo1b.js"`)
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-RtRL5WZ7.js"`)
          expect(chunk.dynamicImports[0]).toMatchInlineSnapshot(
            `"dynamic-GEHD338z.js"`,
          )
          break

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"entry-r47VG7AS.js"`)
          expect(chunk.imports[0]).toMatchInlineSnapshot(`"shared-RtRL5WZ7.js"`)
          expect(chunk.dynamicImports).toStrictEqual([])
          break

        case path.join(__dirname, 'dynamic.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"dynamic-GEHD338z.js"`)
          break

        default:
          break
      }
    }
  },
})
