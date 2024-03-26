import { defineTest } from '@tests/index'

export default defineTest({
  config: {
    resolve: {
      alias: {
        '@': __dirname,
      },
    },
  },
})
