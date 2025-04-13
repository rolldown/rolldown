import { importGlobPlugin } from 'rolldown/experimental'
import { RolldownOutput } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import * as fs from 'node:fs'
import * as path from 'path'

export default defineTest({
  config: {
    plugins: [
      importGlobPlugin({
        restoreQueryExtension: true,
      }),
      {
        name: 'load-file-with-query',
        load(id: string) {
          const [p, _] = id.split('?')
          const res = fs.readFileSync(p, 'utf-8')
          return res
        },
      },
    ],
  },
  async afterTest(output: RolldownOutput) {
    output.output.forEach((chunk) => {
      if (chunk.type === 'chunk') {
        if (chunk.name?.startsWith('index_js')) {
          expect(chunk.code).toMatchFileSnapshot(
            path.resolve(import.meta.dirname, 'dir/index.js.snap'),
          )
        } else if (chunk.name?.startsWith('b_js')) {
          expect(chunk.code).toMatchFileSnapshot(
            path.resolve(import.meta.dirname, 'dir/b.js.snap'),
          )
        }
      }
    })
    await import('./assert.mjs')
  },
})
