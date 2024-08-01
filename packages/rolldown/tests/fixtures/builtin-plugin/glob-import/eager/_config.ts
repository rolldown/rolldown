import { globImportPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [globImportPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
