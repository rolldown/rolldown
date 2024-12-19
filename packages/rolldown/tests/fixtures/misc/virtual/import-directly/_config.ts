import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'virtual-module',
        resolveId(id) {
          if (id === '\0module') {
            return id
          }
        },
        load(id) {
          if (id === '\0module') {
            fn()
            return `export default 'module'`
          }
        },
      },
    ],
  },
  afterTest() {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
