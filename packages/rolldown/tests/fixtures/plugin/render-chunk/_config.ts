import { expect, vi } from 'vitest'
import path from 'node:path'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')

const renderChunkFn = vi.fn()

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (code, chunk) => {
          renderChunkFn()
          expect(code.indexOf('console.log') > -1).toBe(true)
          expect(chunk.name).toBe('main')
          expect(chunk.fileName).toBe('main.js')
          expect(chunk.isEntry).toBe(true)
          expect(chunk.isDynamicEntry).toBe(false)
          expect(chunk.facadeModuleId).toBe(entry)
          expect(chunk.exports.length).toBe(0)
          expect(chunk.imports).toStrictEqual([])
          expect(chunk.moduleIds).toStrictEqual([entry])
          expect(Object.keys(chunk.modules).length).toBe(1)
          expect(Object.values(chunk.modules)[0].code).toBe(
            '//#region main.js\nconsole.log();\n\n//#endregion',
          )
          expect(Object.values(chunk.modules)[0].renderedLength).toBe(46)
          return 'render-chunk-code'
        },
      },
    ],
    output: {
      sourcemap: true,
    },
  },
  afterTest: (output) => {
    expect(renderChunkFn).toHaveBeenCalledTimes(1)
    expect(output.output[0].code.includes('render-chunk-code')).toBe(true)
  },
})
