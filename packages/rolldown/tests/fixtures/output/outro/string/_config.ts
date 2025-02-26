import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const outroText = '/* outro test */\n'
const onLogFn = vi.fn()

export default defineTest({
  config: {
    output: {
      format: 'iife',
      outro: outroText,
    },
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('MISSING_NAME_OPTION_FOR_IIFE_EXPORT')
      onLogFn()
    },
  },
  afterTest() {
    expect(onLogFn).toHaveBeenCalledTimes(1)
  },
})
