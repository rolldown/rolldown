import { defineTest } from '@tests'

export default defineTest({
  config: {
    resolve: {
      alias: [{ name: '@', paths: [__dirname] }],
    },
  },
})
