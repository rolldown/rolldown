import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    onwarn(warning) {
      fn()
      expect(warning.code).toBe('UNRESOLVED_IMPORT')
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
