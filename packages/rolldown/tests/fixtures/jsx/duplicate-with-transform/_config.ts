import { expect, vi } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { getOutputChunk } from 'rolldown-tests/utils'

const fn = vi.fn()
export default defineTest({
  config: {
    input: 'main.jsx',
    jsx: {
      mode: 'classic',
      factory: 'h',
      fragment: 'h.f',
    },
    transform: {
      jsx: 'preserve'
    },
    onwarn(warning) {
      fn()
      expect(warning.code).toBe('DUPLICATE_JSX_CONFIG')
    }
  },
  afterTest(output) {
    expect(fn).toHaveBeenCalledTimes(1)
    expect(getOutputChunk(output)[0].code).contain('<><div /></>')
  },
})
