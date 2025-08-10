import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'
import nodePath from 'node:path'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart(this) {
          const ret = await this.resolve('./sub.js', undefined, { skipSelf: false })
          if (!ret) {
            throw new Error('resolve failed')
          }
          const { id, external } = ret
          expect(external).toBe(false)
          expect(id).toEqual(nodePath.join(import.meta.dirname, 'main.js'))
          fn()
        },
        async resolveId(id) {
          if (id === './sub.js') {
            throw new Error('my error')
          }
          return null
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
