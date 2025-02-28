import { expect, vi } from 'vitest'
import { defineTest } from 'rolldown-tests'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    watch: {
      chokidar: {
        ignored: (path: string, stats: any) =>
          stats?.isFile() && !path.endsWith('.js'),
      },
    },
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('CHOKIDAR_WARNING')
      expect(log.message).toContain(
        'The watch.chokidar option is not supported, please use watch.notify instead.',
      )
      onLogFn()
    },
  },
  afterTest() {
    expect(onLogFn).toHaveBeenCalledTimes(1)
  },
})
