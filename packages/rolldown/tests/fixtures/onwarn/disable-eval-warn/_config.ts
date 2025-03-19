import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    onwarn(_) {
      fn()
    },
    checks: {
      eval: false,
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(0)
  },
})
