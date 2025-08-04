import path from 'node:path'

import { expect } from 'vitest'
import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { getOutputChunk } from 'rolldown-tests/utils'

const entry = path.join(__dirname, './main.js')

const calls: string[] = []

type RolldownOutputChunkWithCustomProperty = RolldownOutputChunk & {
  customProperty: string
}

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
          ;(chunk as RolldownOutputChunkWithCustomProperty).customProperty =
            'customProperty'
          expect(
            (chunk as RolldownOutputChunkWithCustomProperty).customProperty,
          ).toBe('customProperty')
          expect('customProperty' in chunk).toBe(true)
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
      {
        name: 'test-update-sourcemap',
        generateBundle(_options, bundle) {
          const chunk = bundle['main.js'] as RolldownOutputChunk
          const map = chunk.map!
          map.file = 'updated-' + map.file
          map.mappings = ';' + map.mappings
          map.sources.push('updated-source.js')
          map.sourcesContent.push('console.log("updated")')
          map.names.push('updated-name')
          map.x_google_ignoreList = [0]
          map.debugId = 'updated-debugId'
          chunk.map = map
        },
      },
      {
        name: 'test-read-updated-sourcemap',
        generateBundle(_options, bundle) {
          const chunk = bundle['main.js'] as RolldownOutputChunk
          const map = chunk.map!
          expect(map.file).toBe('updated-main.js')
          expect(map.mappings).toMatch(/^;/)
          expect(map.sources.at(-1)).toBe('updated-source.js')
          expect(map.sourcesContent.at(-1)).toBe('console.log("updated")')
          expect(map.names.at(-1)).toBe('updated-name')
          expect(map.x_google_ignoreList).toStrictEqual([0])
          expect(map.debugId).toBe('updated-debugId')
        },
      },
    ],
    output: {
      chunkFileNames: '[name].js',
      sourcemap: true,
      sourcemapDebugIds: true,
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
