import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    plugins: [null, undefined, false],
  },
})
