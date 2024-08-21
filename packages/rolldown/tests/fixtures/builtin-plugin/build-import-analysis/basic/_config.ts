import { buildImportAnalysisPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      buildImportAnalysisPlugin({
        preloadCode: "",
        insertPreload: false
      }),
    ],
  },
  async afterTest() {
    // await import('./assert.mjs')
  },
})
