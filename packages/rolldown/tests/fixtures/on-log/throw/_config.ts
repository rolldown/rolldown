import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    onLog() {
      fn()
      throw new Error('convert log to error')
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
  catchError(err: any) {
    expect(err).toBeInstanceOf(Error)
    expect(err.message).toContain('convert log to error')
  }
})
