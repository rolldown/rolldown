import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    external: [/external/, 'external-a'],
  },
})
