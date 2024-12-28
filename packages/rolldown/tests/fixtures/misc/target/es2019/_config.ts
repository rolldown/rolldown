import { defineTest } from '@tests'

export default defineTest({
  config: {
    target: 'ES2019',
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
