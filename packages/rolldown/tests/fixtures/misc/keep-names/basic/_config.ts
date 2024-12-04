import { defineTest } from '@tests'

export default defineTest({
  config: {
    keepNames: true,
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
