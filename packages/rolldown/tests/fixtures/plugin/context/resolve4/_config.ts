import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const fnA = vi.fn()
const fnB = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'tester',
        async buildEnd() {
          const skipSelfTrue = await this.resolve(
            'test-skip-self-true',
            undefined,
            {
              skipSelf: true,
            },
          )
          expect(skipSelfTrue?.id).toBe('return-by-tester2')
          const skipSelfFalse = await this.resolve(
            'test-skip-self-false',
            undefined,
            {
              skipSelf: false,
            },
          )
          expect(skipSelfFalse?.id).toBe('return-by-tester')
        },
        async resolveId(id) {
          fnA()
          if (id === 'test-skip-self-false') {
            // Prevent recursive call
            return 'return-by-tester'
          }

          if (!id.startsWith('test')) {
            // let `main.js` pass
            return null
          }
          return 'return-by-tester'
        },
      },
      {
        name: 'tester2',
        async resolveId(id) {
          fnB()

          if (!id.startsWith('test')) {
            // let `main.js` pass
            return null
          }
          return 'return-by-tester2'
        },
      },
    ],
  },
  afterTest: () => {
    expect(fnA).toHaveBeenCalledTimes(2)
    expect(fnB).toHaveBeenCalledTimes(2)
  },
})
