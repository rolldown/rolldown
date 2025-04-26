import path from 'node:path'
import { defineTest } from 'rolldown-tests'
import {
  dynamicImportVarsPlugin,
  importGlobPlugin,
} from 'rolldown/experimental'

export default defineTest({
  config: {
    plugins: [
      dynamicImportVarsPlugin({
        exclude: [/main\.js$/]
      }),
      importGlobPlugin({
        root: path.resolve(import.meta.dirname),
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
