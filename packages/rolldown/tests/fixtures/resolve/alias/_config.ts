import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    resolve: {
      alias: {
        '@': ['./not-exists', __dirname],
        'ignore-me': false,
      },
    },
  },
  afterTest() {
    import('./assert.mjs')
  },
})
