import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

let mainWithNameChunkReferenceId: string,
  mainWithFileNameChunkReferenceId: string,
  runOnceRenderChunk = false
const emittedChunkPreliminaryFilenames: string[] = [],
  emittedChunkFilenames: string[] = []

export default defineTest({
  skipComposingJsPlugin: true,
  config: {
    output: {
      entryFileNames: '[name].[hash].js',
    },
    plugins: [
      {
        name: 'test-plugin-context',
        buildStart() {
          mainWithNameChunkReferenceId = this.emitFile({
            type: 'chunk',
            name: 'main-with-name',
            id: './main.js',
          })
          mainWithFileNameChunkReferenceId = this.emitFile({
            type: 'chunk',
            fileName: 'main-with-fileName.js',
            id: './main.js',
          })
          expect(this.getFileName(mainWithFileNameChunkReferenceId)).toBe(
            'main-with-fileName.js',
          )
        },
        renderChunk() {
          if (runOnceRenderChunk) {
            return
          }
          runOnceRenderChunk = true
          emittedChunkPreliminaryFilenames.push(
            this.getFileName(mainWithNameChunkReferenceId),
          )
          emittedChunkPreliminaryFilenames.push(
            this.getFileName(mainWithFileNameChunkReferenceId),
          )
        },
        generateBundle() {
          emittedChunkFilenames.push(
            this.getFileName(mainWithNameChunkReferenceId),
          )
          emittedChunkFilenames.push(
            this.getFileName(mainWithFileNameChunkReferenceId),
          )
        },
      },
    ],
  },
  afterTest: (output) => {
    expect(emittedChunkPreliminaryFilenames).toMatchInlineSnapshot(`
      [
        "main-with-name.!~{001}~.js",
        "main-with-fileName.js",
      ]
    `)
    expect(emittedChunkFilenames).toMatchInlineSnapshot(`
      [
        "main-with-name.gM07keqn.js",
        "main-with-fileName.js",
      ]
    `)
  },
})
