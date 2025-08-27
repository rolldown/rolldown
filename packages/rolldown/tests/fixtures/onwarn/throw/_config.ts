import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    onwarn() {
      fn()
      throw new Error('convert warn to error')
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
  catchError(err: any) {
    expect(err).toBeInstanceOf(Error)
    expect(err.message).toContain('convert warn to error')
  }
})
