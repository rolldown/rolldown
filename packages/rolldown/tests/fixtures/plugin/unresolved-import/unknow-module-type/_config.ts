import path from 'node:path'
import { expect, vi } from 'vitest'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')

const resolveIdFn = vi.fn()

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        resolveId(id: string) {
          resolveIdFn()
          if (id === 'test.javascript') {
            return id
          }
        },
      },
    ],
  },
  onerror: (err: unknown) => {
    expect(err!.toString()).toBe('Error: Build failed')
    expect(resolveIdFn).toHaveBeenCalledTimes(2)
  },
})
