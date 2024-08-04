import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const buildStartFn = vi.fn()
const buildStartFn2 = vi.fn()
const sleepAsync = (ms: number) =>
  new Promise((resolve) => setTimeout(resolve, ms))

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-1',
        async buildStart() {
          await sleepAsync(100)
          buildStartFn()
        },
        transform() {
          expect(buildStartFn).toHaveBeenCalledTimes(1)
          expect(buildStartFn2).toHaveBeenCalledTimes(1)
        },
      },
      {
        name: 'test-plugin-2',
        async buildStart(config) {
          await sleepAsync(100)
          buildStartFn2()
        },
      },
    ],
  },
})
