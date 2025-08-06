import { aliasPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      aliasPlugin({
        entries: [{ find: 'rolldown', replacement: '.' }],
      }),
    ],
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('UNRESOLVED_IMPORT')
      expect(log.message).toContain(
        "Could not resolve 'rolldownlib.js' in main.js",
      )
      expect(log.plugin).toBeUndefined()
      onLogFn()
    },
  },
  async afterTest() {
    expect(onLogFn).toHaveBeenCalledTimes(1)

    try {
      await import('./assert.mjs')
    } catch (err: any) {
      expect(err.toString()).contains(
        `Cannot find package 'rolldownlib.js'`,
      )
    }
  },
})
