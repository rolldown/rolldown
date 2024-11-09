import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [null, undefined, false],
  },
})
