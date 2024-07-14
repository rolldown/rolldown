// cSpell:disable
import { expect, vi } from 'vitest'
import { defineTest } from '@tests'
import { getOutputChunk } from '@tests/utils'
import path from 'node:path'

const fn = vi.fn()

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
        augmentChunkHash: (chunk) => {
          fn()
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
          expect(chunk.fileName).toMatchInlineSnapshot(`"main-M-2YP1Eg.js"`)
          break

        case path.join(__dirname, 'entry.js'):
          expect(chunk.fileName).toMatchInlineSnapshot(`"entry-LZxEycPx.js"`)
          break

        default:
          break
      }
    }
  },
})
