import { defineTest } from 'rolldown-tests'
import * as fs from 'fs'
import { expect, vi } from 'vitest'

const onLogFn = vi.fn()

export default defineTest({
  config: {
    input: './main.jsx',
    plugins: [
      {
        name: 'test-plugin',
        load: function (id) {
          let code = fs.readFileSync(id).toString()
          return {
            code,
          }
        },
      },
    ],
    onLog(level, log) {
      expect(level).toBe('warn')
      expect(log.code).toBe('UNRESOLVED_IMPORT')
      expect(log.message).toContain(
        "Could not resolve 'react/jsx-runtime' in main.jsx",
      )
      onLogFn()
    },
  },
  afterTest() {
    expect(onLogFn).toHaveBeenCalledTimes(1)
  },
})
