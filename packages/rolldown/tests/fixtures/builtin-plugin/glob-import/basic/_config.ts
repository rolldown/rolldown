import { globImportPlugin } from 'rolldown'
import { defineTest } from '@tests'
import * as path from 'path'

export default defineTest({
  config: {
    plugins: [
      globImportPlugin({
        root: path.resolve(import.meta.dirname),
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
