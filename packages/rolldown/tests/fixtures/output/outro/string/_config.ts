import { defineTest } from '@tests'

const outroText = '/* outro test */\n'

export default defineTest({
  config: {
    output: {
      format: 'iife',
      outro: outroText,
    },
  },
})
