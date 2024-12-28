import { defineTest } from '@tests'

export default defineTest({
  config: {
    target: 'ES2015',
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
