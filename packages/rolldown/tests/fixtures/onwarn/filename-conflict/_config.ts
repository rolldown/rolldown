import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    onwarn(warning) {
      fn()
      expect(warning.code).toBe('FILE_NAME_CONFLICT')
    },
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          this.emitFile({ type: 'asset', source: 'foo1', fileName: 'foo.txt' })
          this.emitFile({ type: 'asset', source: 'foo2', fileName: 'Foo.txt' })
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
