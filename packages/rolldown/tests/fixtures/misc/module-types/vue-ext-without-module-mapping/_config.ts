import { defineTest } from '@tests'

export default defineTest({
  config: {},
  afterTest: async () => {
    // @ts-ignore
    await import('./assert.mjs')
  },
})
