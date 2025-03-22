import * as path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { importGlobPlugin } from 'rolldown/experimental'

const root = path.join(
  path.dirname(path.resolve(import.meta.dirname)),
  'fixtures/a',
)

export default defineTest({
  config: {
    input: './index.ts',
    output: {
      chunkFileNames: '[name].js',
    },
    treeshake: false,
    plugins: [
      importGlobPlugin({ root }),
      {
        name: 'virtual:module',
        resolveId(id) {
          if (id === 'virtual:module') {
            return 'virtual:module'
          }
        },
        load(id) {
          if (id === 'virtual:module') {
            // TODO: support importGlob in virtual module
            // const code = [
            //   "export const a = import.meta.glob('/modules/*.ts')",
            //   "export const b = import.meta.glob(['/../fixture-b/*.ts'])",
            // ].join('\n')
            // return code
            return 'export const a = 1; export const b = 0'
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
