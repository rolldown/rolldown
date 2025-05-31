import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    input: 'main.ts',
    keepNames: true,
    resolve: {
      tsconfigFilename: 'tsconfig.json',
    },
  },
  async afterTest(_output) {
    await import('./assert.mjs')
  },
})
