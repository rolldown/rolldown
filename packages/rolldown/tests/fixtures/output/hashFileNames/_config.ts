import { defineTest } from '@tests'
import { expect } from 'vitest'
import { getOutputChunk } from '@tests/utils'

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
    expect(renderChunks.map((chunk) => chunk.fileName)).toMatchInlineSnapshot(`
      [
        "main-!~{000}~.js",
        "entry-!~{001}~.js",
        "shared-!~{002}~.js",
      ]
    `)
    expect(renderChunks.map((chunk) => chunk.imports)).toMatchInlineSnapshot(`
      [
        [
          "shared-!~{002}~.js",
        ],
        [
          "shared-!~{002}~.js",
        ],
        [],
      ]
    `)

    // The `OutputChunk` file names hash placeholder should be replaced.
    const chunks = getOutputChunk(output)
    expect(chunks.map((chunk) => chunk.fileName)).toMatchInlineSnapshot(`
      [
        "main-oEQ4m6Um.js",
        "entry-6EYT-FfB.js",
        "shared-RtRL5WZ7.js",
      ]
    `)
    expect(chunks.map((chunk) => chunk.imports)).toMatchInlineSnapshot(`
      [
        [
          "shared-RtRL5WZ7.js",
        ],
        [
          "shared-RtRL5WZ7.js",
        ],
        [],
      ]
    `)
  },
})
