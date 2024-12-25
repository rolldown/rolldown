import { defineTest } from '@tests'
import { expect } from 'vitest'
import path from 'node:path'
import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { getOutputChunk } from '@tests/utils'

const entry = path.join(__dirname, './main.js')

const calls: string[] = []

export default defineTest({
  config: {
    input: [entry, path.join(__dirname, './index.js')],
    plugins: [
      {
        name: 'test-plugin',
        generateBundle: async (_options, bundle, isWrite) => {
          const chunk = bundle['main.js'] as RolldownOutputChunk
          expect(chunk.code.indexOf('console.log') > -1).toBe(true)
          expect(chunk.type).toBe('chunk')
          expect(chunk.fileName).toBe('main.js')
          expect(chunk.isEntry).toBe(true)
          expect(chunk.isDynamicEntry).toBe(false)
          expect(chunk.facadeModuleId).toBe(entry)
          expect(chunk.exports.length).toBe(0)
          expect(chunk.imports).toStrictEqual(['share.js'])
          expect(chunk.moduleIds).toStrictEqual([entry])
          expect(Object.keys(chunk.modules).length).toBe(1)
          expect(Object.values(chunk.modules)[0].code).toBe(
            '//#region main.js\nconsole.log();\n\n//#endregion',
          )
          expect(Object.values(chunk.modules)[0].renderedLength).toBe(46)
          expect(chunk.map).toBeDefined()
          expect(chunk.map!.toString()).toContain('"version":')
          // called bundle.generate()
          expect(isWrite).toBe(true)
          // Mutate chunk
          chunk.code = 'console.error()'
          // Delete chunk
          delete bundle['index.js']
          delete bundle['share.js']

          await new Promise((resolve) => setTimeout(resolve, 100))

          calls.push('test-plugin')
        },
      },
      {
        name: 'test-plugin-2',
        generateBundle: (_options, _bundle, _isWrite) => {
          calls.push('test-plugin-2')
        },
      },
    ],
    output: {
      chunkFileNames: '[name].js',
      sourcemap: true,
    },
  },
  beforeTest: () => {
    calls.length = 0
  },
  afterTest: (output) => {
    expect(calls).toStrictEqual(['test-plugin', 'test-plugin-2'])
    const chunks = getOutputChunk(output)
    expect(chunks.length).toBe(1)
    expect(chunks[0].code).toBe('console.error()')
    expect(Object.values(chunks[0].modules)[0].code).toBe(
      '//#region main.js\nconsole.log();\n\n//#endregion',
    )
  },
})
