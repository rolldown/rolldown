import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const bannerTxt = '/* banner */'
const banner = async () => bannerTxt

export default defineTest({
  config: {
    output: {
      banner,
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code.startsWith(bannerTxt)).toBe(true)
  },
})
