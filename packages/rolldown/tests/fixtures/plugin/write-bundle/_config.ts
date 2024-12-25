import { expect } from 'vitest'
import path from 'node:path'
import fs from 'node:fs'
import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')
const foo = path.join(__dirname, './foo.js')

const calls: string[] = []

export default defineTest({
  config: {
    input: entry,
    treeshake: false,
    plugins: [
      {
        name: 'test-plugin',
        writeBundle: async (_options, bundle) => {
          // Make sure the bundle already write to disk.
          expect(fs.existsSync(path.resolve(__dirname, 'main.js'))).toBe(true)

          const chunk = bundle['main.js'] as RolldownOutputChunk
          expect(chunk.code.indexOf('console.log') > -1).toBe(true)
          expect(chunk.type).toBe('chunk')
          expect(chunk.name).toBe('main')
          expect(chunk.fileName).toBe('main.js')
          expect(chunk.isEntry).toBe(true)
          expect(chunk.isDynamicEntry).toBe(false)
          expect(chunk.facadeModuleId).toBe(entry)
          expect(chunk.exports.length).toBe(0)
          expect(chunk.imports).length(0)
          // The `foo.js` should be include `modules/moduleIds` even it is empty.
          expect(chunk.moduleIds).toStrictEqual([foo, entry])
          expect(Object.keys(chunk.modules).length).toStrictEqual(2)

          await new Promise((resolve) => setTimeout(resolve, 100))
          calls.push('test-plugin')
        },
      },
      {
        name: 'test-plugin-2',
        writeBundle: () => {
          calls.push('test-plugin-2')
        },
      },
    ],
  },
  beforeTest: () => {
    calls.length = 0
  },
  afterTest: () => {
    expect(calls).toStrictEqual(['test-plugin', 'test-plugin-2'])
  },
})
