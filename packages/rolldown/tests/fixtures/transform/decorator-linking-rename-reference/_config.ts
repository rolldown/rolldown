import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    input: 'main.ts',
    resolve: {
      tsconfigFilename: 'tsconfig.json',
    },
    keepNames: true,
  },
  async afterTest(_output) {
    await import('./assert.mjs')
  },
})
