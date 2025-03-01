import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const introText = '/* intro test */\n'
const intro = () => introText

export default defineTest({
  config: {
    output: {
      intro,
    },
  },
  afterTest(output) {
    expect(output.output[0].code.includes(introText)).toBe(true)
  },
})
