import { defineTest } from '@tests'

export default defineTest({
  config: {
    experimental: {
      strictExecutionOrder: true,
    },
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
