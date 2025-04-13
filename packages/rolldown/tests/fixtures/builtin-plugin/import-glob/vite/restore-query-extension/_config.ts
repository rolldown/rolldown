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
      importGlobPlugin({ root, restoreQueryExtension: true }),
      {
        name: 'load-file-with-query',
        load(id: string) {
          if (id.includes('?raw')) {
            const res = fs.readFileSync(id.split('?')[0], 'utf-8')
            return `export default ${JSON.stringify(res)}`
          }
          if (id.includes('?url')) {
            return `export default '/path/to/module.js'`
          }
          if (id.includes('?base64')) {
            const res = fs.readFileSync(id.split('?')[0], 'utf-8')
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
