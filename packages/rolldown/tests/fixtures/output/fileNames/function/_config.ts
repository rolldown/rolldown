import type { RolldownOutputChunk, PreRenderedChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

let preRenderedEntry: PreRenderedChunk
let preRenderedChunk: PreRenderedChunk

export default defineTest({
  config: {
    output: {
      entryFileNames: (chunk) => {
        preRenderedEntry = chunk
        return '[name]-test.js'
      },
      chunkFileNames: (chunk) => {
        preRenderedChunk = chunk
        return '[name]-chunk.js'
      },
    },
  },
  afterTest: (output) => {
    expect(
      output.output.find((chunk) => (chunk as RolldownOutputChunk).isEntry)
        ?.fileName,
    ).toBe('main-test.js')
    expect(
      output.output.find((chunk) => !(chunk as RolldownOutputChunk).isEntry)
        ?.fileName,
    ).toBe('test-chunk.js')

    expect(preRenderedEntry).toMatchObject({
      name: 'main',
      isEntry: true,
      isDynamicEntry: false,
      exports: [],
      facadeModuleId: expect.stringMatching(/main\.js$/),
      moduleIds: [expect.stringMatching(/main\.js$/)],
    })

    expect(preRenderedChunk).toMatchObject({
      name: 'test',
      isEntry: false,
      isDynamicEntry: true,
      exports: ['hello'],
      facadeModuleId: expect.stringMatching(/test\.js$/),
      moduleIds: [expect.stringMatching(/test\.js$/)],
    })
  },
})
