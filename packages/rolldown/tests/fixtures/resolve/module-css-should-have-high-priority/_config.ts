import { defineTest } from '@tests'

export default defineTest({
  config: {
    moduleTypes: {
      // make sure that `.module.css` is matched before `.css`
      '.module.css': 'empty',
    },
  },
  afterTest() {
    // @ts-ignore
    import('./assert.mjs')
  },
})
