import { defineTest } from '@tests'

export default defineTest({
  config: {},
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
