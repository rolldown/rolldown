import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'
import path from 'node:path'
import { RenderedChunk } from 'rolldown'

const renderChunks: RenderedChunk[] = []

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
        renderChunk: (_code, chunk) => {
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
          expect(chunk.fileName).toMatch('main-!~{000}~.js')
          expect(chunk.imports).toMatchObject(['shared-!~{002}~.js'])
          expect(chunk.dynamicImports).toMatchObject(['dynamic-!~{004}~.js'])
          break

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatch('entry-!~{001}~.js')
          expect(chunk.imports).toMatchObject(['shared-!~{002}~.js'])
          expect(chunk.dynamicImports).toStrictEqual([])
          break

        case path.join(__dirname, 'dynamic.js'):
          expect(chunk.fileName).toMatch('dynamic-!~{004}~.js')
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
          expect(chunk.fileName).toMatchInlineSnapshot(`"main-BKJjLIpN.js"`)
          expect(chunk.imports).toMatchInlineSnapshot(
            `
            [
              "shared-4ttuH-iD.js",
            ]
          `,
          )
          expect(chunk.dynamicImports).toMatchInlineSnapshot(
            `
            [
              "dynamic-CtS_O3mS.js",
            ]
          `,
          )
          break

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"entry--UVBFch3.js"`)
          expect(chunk.imports).toMatchInlineSnapshot(
            `
            [
              "shared-4ttuH-iD.js",
            ]
          `,
          )
          expect(chunk.dynamicImports).toStrictEqual([])
          break

        case path.join(__dirname, 'dynamic.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"dynamic-CtS_O3mS.js"`)
          break

        default:
          break
      }
    }
  },
})
