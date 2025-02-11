import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

const introText = '/* intro test */\n'

export default defineTest({
  config: {
    output: {
      format: 'iife',
      intro: introText,
    },
  },
})
