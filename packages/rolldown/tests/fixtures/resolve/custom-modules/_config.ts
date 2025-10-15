import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    resolve: {
      modules: ['custom-node-modules'],
    },
  },
})
