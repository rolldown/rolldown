import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const footerTxt = '// footer test\n'

export default defineTest({
  config: {
    output: {
      footer:footerTxt
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.endsWith(footerTxt)).toBe(true)
  },
})
