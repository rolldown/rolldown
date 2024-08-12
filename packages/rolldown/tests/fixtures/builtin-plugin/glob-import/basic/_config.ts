import { importGlobPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'
import * as path from 'path'

export default defineTest({
  config: {
    plugins: [
      importGlobPlugin({
        root: path.resolve(import.meta.dirname),
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
