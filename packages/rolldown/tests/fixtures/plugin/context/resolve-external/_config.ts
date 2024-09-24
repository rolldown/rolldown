import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    external: ['external'],
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart() {
          const ret = await this.resolve('external')
          if (!ret) {
            throw new Error('resolve failed')
          }
          expect(ret.external).toBe(true)
          expect(ret.id).toBe('external')
          fn()
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
