import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart(this) {
          await this.resolve('./sub.js', undefined, { skipSelf: false })
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
    expect(fn).not.toHaveBeenCalled()
  },
  catchError(err: any) {
    expect(err).toBeInstanceOf(Error)
    expect(err.message).toContain('Errored while resolving "./sub.js" in `this.resolve`.')
    expect(err.message).toContain('Caused by:')
    expect(err.message).toContain('Error: my error')
  },
})
