import { defineTest } from 'rolldown-tests'

const outroText = '/* outro test */\n'

export default defineTest({
  config: {
    output: {
      format: 'iife',
      outro: outroText,
    },
  },
})
