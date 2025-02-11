import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    moduleTypes: {
      // make sure that `.module.css` is matched before `.css`
      '.module.css': 'empty',
    },
  },
  afterTest() {
    import('./assert.mjs')
  },
})
