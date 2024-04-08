import { defineTest } from '@tests'

export default defineTest({
  config: {
    resolve: {
      alias: [{ find: '@', replacement: __dirname }],
    },
  },
})
