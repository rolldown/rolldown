import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()

export default defineTest({
  config: {
    onLog(_level, log) {
      fn(log)
    },
  },
  beforeTest: () => {
    fn.mockClear()
  },
  afterTest: () => {
    const log = fn.mock.calls[0][0]
    // spread object to test enumerable properties
    expect({ ...log }).toEqual({
      code: 'UNRESOLVED_IMPORT',
      exporter: '@rolldown/test-unresolved-import',
      id: expect.stringContaining('main.js'),
      message: expect.any(String),
    })
  },
})
