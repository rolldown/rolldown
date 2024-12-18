import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        onLog: (level, log) => {
          expect(level).toBe('warn')
          expect(log.code).toBe('CYCLE_LOADING')
          expect(log.message).toContain(
            'cycle loading at test-plugin-context plugin',
          )
          onLogFn()
        },
        async load(id) {
          this.load({ id })
        },
      },
    ],
  },
  afterTest: () => {
    expect(onLogFn).toHaveBeenCalledTimes(1)
  },
})
