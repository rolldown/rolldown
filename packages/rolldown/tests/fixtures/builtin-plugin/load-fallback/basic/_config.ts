import { loadFallbackPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    plugins: [loadFallbackPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
