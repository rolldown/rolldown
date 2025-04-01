import { expect, vi } from 'vitest'
import path from 'node:path'
import { defineTest } from 'rolldown-tests'
import { RenderedChunk } from 'rolldown'

const entry = path.join(__dirname, './main.js')
const foo = path.join(__dirname, './foo.js')

const renderChunkFn = vi.fn()

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (code, chunk, _options, meta) => {
          renderChunkFn()
          expect(code.indexOf('console.log') > -1).toBe(true)
          testChunk(chunk)
          testChunk(meta.chunks['main.js'])

          function testChunk(chunk: RenderedChunk) {
            expect(chunk.name).toBe('main')
            expect(chunk.fileName).toBe('main.js')
            expect(chunk.isEntry).toBe(true)
            expect(chunk.isDynamicEntry).toBe(false)
            expect(chunk.facadeModuleId).toBe(entry)
            expect(chunk.exports.length).toBe(0)
            expect(chunk.imports).toStrictEqual([])
            expect(chunk.moduleIds).toStrictEqual([foo, entry])
            expect(Object.keys(chunk.modules).length).toBe(2)
            for (const [moduleId, module] of Object.entries(chunk.modules)) {
              switch (moduleId) {
                case entry:
                  expect(module.code).toBe(
                    '//#region main.js\nconsole.log(foo);\n\n//#endregion\n',
                  )
                  expect(module.renderedLength).toBe(50)
                  break

                case foo:
                  expect(module.renderedExports).toStrictEqual(['foo']) // The `unused` export is removed
                  break
                default:
                  break
              }
            }
          }

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
