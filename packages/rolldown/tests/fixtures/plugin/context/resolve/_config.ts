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
          const ret = await this.resolve('./main.js')
          if (!ret) {
            throw new Error('resolve failed')
          }
          const { id, external } = ret
          expect(external).toBe(false)
          expect(id).toEqual(nodePath.join(import.meta.dirname, 'main.js'))
          fn()
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
