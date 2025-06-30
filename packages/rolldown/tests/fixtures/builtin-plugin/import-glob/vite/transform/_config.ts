import * as fs from 'node:fs'
import * as path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { importGlobPlugin } from 'rolldown/experimental'

const root = path.join(
  path.dirname(path.resolve(import.meta.dirname)),
  'fixtures',
)

export default defineTest({
  config: {
    input: '../fixtures/a/index.ts',
    output: {
      chunkFileNames: '[name].js',
    },
    plugins: [
      importGlobPlugin({ root }),
      {
        name: 'load-file-with-query',
        resolveId(id) {
          if (id.startsWith('/')) {
            return path.join(root, id)
          }
        },
        load(id: string) {
          if (id.endsWith('?raw')) {
            const res = fs.readFileSync(id.slice(0, -4), 'utf-8')
            return `export default ${JSON.stringify(res)}`
          }
          if (id.endsWith('?url')) {
            return `export default '/path/to/module.js'`
          }
          if (id.endsWith('?base64')) {
            const res = fs.readFileSync(id.slice(0, -7), 'utf-8')
            return `export default ${JSON.stringify(btoa(res))}`
          }
        },
      },
    ],
  },
  async afterTest(output) {
    await expect(output.output[0].code).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'index.ts.snap'),
    )
  },
})
