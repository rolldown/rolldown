import { importGlobPlugin } from 'rolldown/experimental'
import { RolldownOutput } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import * as fs from 'node:fs'
import * as path from 'node:path'

// ported from https://github.com/vitejs/vite/tree/021443c5a11eedfb176f2c65d5fab65e9c21adfb/packages/vite/src/node/__tests__/plugins/importGlob
export default defineTest({
  config: {
    input: './fixture-a/index.ts',
    output: {
      chunkFileNames: '[name].js',
    },
    plugins: [
      importGlobPlugin({
        root: path.resolve(import.meta.dirname),
      }),
      {
        name: 'load-file-with-query',
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
  async afterTest(output: RolldownOutput) {
    await expect(output.output[0].code).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'fixture-a/index.ts.snap'),
    )
  },
})
