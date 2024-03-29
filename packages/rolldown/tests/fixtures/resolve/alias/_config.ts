import { defineTest } from '@tests'

export default defineTest({
  config: {
    resolve: {
      alias: {
        '@': __dirname,
      },
    },
  },
})
