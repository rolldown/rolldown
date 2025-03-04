import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const outroText = '/* outro test */\n'
const outro = () => outroText

export default defineTest({
  config: {
    output: {
      outro,
    },
  },
  afterTest(output) {
    expect(output.output[0].code.includes(outroText)).toBe(true)
  },
})
