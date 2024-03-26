import type { RollupOptions, RollupOutput } from 'rolldown'
import { expect, vi } from 'vitest'
import path from 'node:path'

const entry = path.join(__dirname, './main.js')

const renderChunkFn = vi.fn()

const config: RollupOptions = {
  input: entry,
  plugins: [
    {
      name: 'test-plugin',
      renderChunk: (code, chunk) => {
        renderChunkFn()
        expect(code.indexOf('console.log') > -1).toBe(true)
        expect(chunk.type).toBe('chunk')
        expect(chunk.fileName).toBe('main.js')
        expect(chunk.isEntry).toBe(true)
        expect(chunk.isDynamicEntry).toBe(false)
        expect(chunk.facadeModuleId).toBe(entry)
        expect(chunk.exports.length).toBe(0)
        expect(chunk.moduleIds).toStrictEqual([entry])
        expect(Object.keys(chunk.modules).length).toBe(1)
        return 'render-chunk-code'
      },
    },
  ],
}

export default {
  config,
  afterTest: (output: RollupOutput) => {
    expect(renderChunkFn).toHaveBeenCalledTimes(1)
    expect(output.output[0].code).toBe('render-chunk-code')
  },
}
