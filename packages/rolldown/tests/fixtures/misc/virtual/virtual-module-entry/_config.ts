import { defineTest } from '@tests'

export default defineTest({
  config: {
    input: ['\0virtual:test'],
    plugins: [
      {
        resolveId(id) {
          if (id === '\0virtual:test') {
            return id
          }
        },
        load(id) {
          if (id === '\0virtual:test') {
            return `export const a = 1`
          }
        },
      },
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
