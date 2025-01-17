import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'
import path from 'node:path'

const entry = path.join(__dirname, './main.js')

const resolveIdFn = vi.fn()

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        resolveId: function (id, importer, options) {
          resolveIdFn()
          if (id === 'external') {
            expect(importer).toStrictEqual(entry)
            expect(options).toMatchObject({
              isEntry: false,
              kind: 'require-call',
            })
            return {
              id,
              external: true,
            }
          }
          if (id === './foo') {
            expect(importer).toStrictEqual(entry)
            expect(options).toMatchObject({
              isEntry: false,
              kind: 'import-statement',
            })
            return {
              id: path.join(__dirname, './foo.js'),
              external: false,
            }
          }
          if (id === 'dynamic') {
            expect(importer).toStrictEqual(entry)
            expect(options).toMatchObject({
              isEntry: false,
              kind: 'dynamic-import',
            })
            return {
              id,
              external: true,
            }
          }
          if (id === entry) {
            expect(importer).toBeUndefined()
            expect(options).toMatchObject({
              isEntry: true,
              kind: 'import-statement',
            })
          }
        },
      },
    ],
  },
  afterTest: () => {
    expect(resolveIdFn).toHaveBeenCalledTimes(4)
  },
})
