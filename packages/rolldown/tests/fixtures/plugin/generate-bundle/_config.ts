import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import path from 'node:path'
import { OutputChunk } from 'rollup'

const entry = path.join(__dirname, './main.js')

const generateBundleFn = vi.fn()

export default defineTest({
  skip: true,
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        // @ts-expect-error
        generateBundle: (options, bundle, isWrite) => {
          generateBundleFn()
          const chunk = bundle['main.js'] as OutputChunk
          expect(chunk.code.indexOf('console.log') > -1).toBe(true)
          expect(chunk.type).toBe('chunk')
          expect(chunk.fileName).toBe('main.js')
          expect(chunk.isEntry).toBe(true)
          expect(chunk.isDynamicEntry).toBe(false)
          expect(chunk.facadeModuleId).toBe(entry)
          expect(chunk.exports.length).toBe(0)
          expect(chunk.moduleIds).toStrictEqual([entry])
          expect(Object.keys(chunk.modules).length).toBe(1)
          // called bundle.generate()
          expect(isWrite).toBe(true)
        },
      },
    ],
  },
  afterTest: () => {
    expect(generateBundleFn).toHaveBeenCalledTimes(1)
  },
})
