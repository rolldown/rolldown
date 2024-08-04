import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    onwarn(warning) {
      fn()
      expect(warning.code).toBe('CIRCULAR_DEPENDENCY')
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
    fn.mockReset()
  },
})
