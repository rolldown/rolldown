import { defineTest } from '@tests'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      entryFileNames: '[name].mjs',
      exports: 'named',
      name: 'module',
      format: 'umd',
    },
  },
  afterTest: async () => {
    await import('./assert.mjs')
  },
})
