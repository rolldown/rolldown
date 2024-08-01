import {
  dynamicImportVarsPlugin,
  globImportPlugin,
} from 'rolldown/experimental'
import { defineTest } from '@tests'
import path from 'path'

export default defineTest({
  config: {
    plugins: [
      dynamicImportVarsPlugin(),
      globImportPlugin({
        root: path.resolve(import.meta.dirname),
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
