import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async load(id) {
          this.load({ id })
        },
      },
    ],
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('CYCLE_LOADING')
      expect(log.message).toContain(
        'cycle loading at test-plugin-context plugin',
      )
      expect(log.plugin).toBeUndefined()
      onLogFn()
    },
  },
  afterTest: () => {
    expect(onLogFn).toHaveBeenCalledTimes(1)
  },
})
