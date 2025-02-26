import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    keepNames: true,
    external: ['node:assert'],
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
