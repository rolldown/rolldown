import { expect, vi } from 'vitest'
import path from 'node:path'
import type { RolldownOutputChunk } from '../../../../src'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')

const writeBundleFn = vi.fn()

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        writeBundle: (_options, bundle) => {
          writeBundleFn()
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
          expect(chunk.moduleIds).toStrictEqual([entry])
          expect(Object.keys(chunk.modules).length).toBe(1)
        },
      },
    ],
  },
  afterTest: () => {
    expect(writeBundleFn).toHaveBeenCalledTimes(1)
  },
})
