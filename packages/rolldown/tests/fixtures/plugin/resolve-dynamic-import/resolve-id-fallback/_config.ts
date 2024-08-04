import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import path from 'node:path'

const entry = path.join(__dirname, './main.js')

const resolveId = vi.fn()

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        resolveId: function (id, importer, options) {
          resolveId()
          if (id === 'foo') {
            expect(importer).toStrictEqual(entry)
            return {
              id: path.join(__dirname, './foo.js'),
            }
          }
        },
      },
    ],
  },
  afterTest: () => {
    expect(resolveId).toHaveBeenCalledTimes(2)
  },
})
