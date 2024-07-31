import { BuiltinGlobImportPlugin } from 'rolldown'
import { defineTest } from '@tests'
import * as path from 'path'

export default defineTest({
  config: {
    plugins: [
      new BuiltinGlobImportPlugin({
        root: path.resolve(import.meta.dirname),
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
