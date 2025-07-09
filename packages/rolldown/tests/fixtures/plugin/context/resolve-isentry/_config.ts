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
          const ret = await this.resolve('./sub.js', undefined, { isEntry: true })
          if (!ret) {
            throw new Error('resolve failed')
          }
        },
      },
      {
        name: 'test-plugin-isentry',
        resolveId(id, _importer, options) {
          if (id === './sub.js') {
            expect(options.isEntry).toBe(true)
            fn()
            return nodePath.resolve(import.meta.dirname, 'main.js')
          }
        }
      }
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
