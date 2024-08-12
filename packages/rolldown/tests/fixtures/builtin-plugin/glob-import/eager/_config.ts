import { importGlobPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [importGlobPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
