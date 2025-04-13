import { importGlobPlugin } from 'rolldown/experimental'
import { RolldownOutput } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import * as fs from 'node:fs'
import * as path from 'path'

export default defineTest({
  config: {
    plugins: [
      importGlobPlugin({}),
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
    output.output.forEach(async (chunk) => {
      if (chunk.type === 'chunk') {
        switch (chunk.name) {
          case 'b': {
            await expect(chunk.code).toMatchFileSnapshot(
              path.resolve(import.meta.dirname, 'dir/b.js.snap'),
            )
            break
          }
          case 'dir_index': {
            await expect(chunk.code).toMatchFileSnapshot(
              path.resolve(import.meta.dirname, 'dir/index.js.snap'),
            )
            break
          }
        }
      }
    })
    await import('./assert.mjs')
  },
})
