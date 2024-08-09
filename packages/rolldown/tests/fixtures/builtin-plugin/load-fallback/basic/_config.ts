import { loadFallbackPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [loadFallbackPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
