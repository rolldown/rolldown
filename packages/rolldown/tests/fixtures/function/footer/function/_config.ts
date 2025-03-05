import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const footerTxt = '// footer test\n'
const footer = () => footerTxt

export default defineTest({
  config: {
    output: {
      footer,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.endsWith(footerTxt)).toBe(true)
  },
})
