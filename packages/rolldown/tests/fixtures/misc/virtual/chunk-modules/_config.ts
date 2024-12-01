import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        resolveId(id) {
          if (id === '\0module') {
            return id
          }
        },
        load(id) {
          if (id === '\0module') {
            return `export default '[ok]'`
          }
        },
        renderChunk(_, chunk) {
          fn(chunk.modules['\0module'].code)
        },
      },
    ],
  },
  afterTest(output) {
    expect(fn.mock.calls[0][0]).toContain('[ok]')
    expect(output.output[0].modules['\0module'].code).toContain('[ok]')
  },
})
