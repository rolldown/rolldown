import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    keepNames: true,
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
