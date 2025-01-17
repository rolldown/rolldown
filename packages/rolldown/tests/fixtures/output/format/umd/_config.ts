import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      name: 'module',
      format: 'umd',
      entryFileNames: '[name].cjs',
    },
  },
  afterTest: async () => {
    await import('./assert.cjs')
  },
})
