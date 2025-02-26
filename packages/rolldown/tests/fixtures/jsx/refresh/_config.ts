import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'
import { getOutputChunk } from 'rolldown-tests/utils'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    input: 'main.jsx',
    jsx: {
      refresh: true,
    },
    external: ['react'],
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('UNRESOLVED_IMPORT')
      expect(log.message).toContain(
        "Could not resolve 'react/jsx-runtime' in main.jsx",
      )
      onLogFn()
    },
  },
  afterTest: (output) => {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    const chunk = getOutputChunk(output)[0]
    expect(chunk.code.includes('$RefreshReg$')).toBe(true)
  },
})
