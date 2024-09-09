import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  skip: true, // TODO: enable this test when `InputOptions.checks` is implemented
  config: {
    onwarn(warning) {
      fn()
      expect(warning.code).toBe('CIRCULAR_DEPENDENCY')
    },
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
