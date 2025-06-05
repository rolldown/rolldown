import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    experimental: {
      hmr: {},
    },
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
