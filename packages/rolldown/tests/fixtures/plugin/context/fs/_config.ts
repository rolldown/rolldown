import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async load(id) {
          if (id.endsWith('main.js')) {
            fn()
            const stat = await this.fs.stat(id)
            expect(stat.isFile()).toBe(true)
          }
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
