import { defineTest } from '@tests'

export default defineTest({
  config: {
    resolve: {
      alias: {
        '@': ['./not-exists', __dirname],
      },
    },
  },
})
