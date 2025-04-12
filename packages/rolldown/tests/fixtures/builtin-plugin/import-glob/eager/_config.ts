import { importGlobPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    plugins: [importGlobPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
