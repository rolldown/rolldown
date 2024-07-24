import type { RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

const outroText = '/* outro test */\n'

export default defineTest({
  config: {
    format: 'iife',
    output: {
      outro: outroText,
    },
  },
})
