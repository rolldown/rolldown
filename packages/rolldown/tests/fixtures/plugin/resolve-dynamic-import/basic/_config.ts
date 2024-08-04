import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import path from 'node:path'

const entry = path.join(__dirname, './main.js')

const resolveDynamicImport = vi.fn()

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        resolveDynamicImport: function (id, importer) {
          resolveDynamicImport()
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
  afterTest: (output) => {
    expect(resolveDynamicImport).toHaveBeenCalledTimes(1)
  },
})
