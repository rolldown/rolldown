import { expect, vi } from 'vitest'
import { defineTest } from 'rolldown-tests'

const renderChunkFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'testCodeFilter',
        renderChunk: {
          filter: {
            code: {
              include: ['hello world'],
            },
          },
          handler(_) {
            renderChunkFn()
            return null
          },
        },
      },
    ],
  },
  afterTest: () => {
    expect(renderChunkFn).toHaveBeenCalledTimes(0)
  },
})
