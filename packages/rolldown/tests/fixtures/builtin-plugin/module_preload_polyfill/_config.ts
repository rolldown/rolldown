import { modulePreloadPolyfillPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [modulePreloadPolyfillPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
