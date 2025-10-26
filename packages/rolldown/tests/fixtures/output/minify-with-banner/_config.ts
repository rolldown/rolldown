import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

const bannerTxt = '// ==UserScript==\n// @name My Script\n// ==/UserScript==\n'
const footerTxt = '// end of script\n'

export default defineTest({
  config: {
    output: {
      banner: bannerTxt,
      intro: '// intro comment',
      outro: '// outro comment',
      footer: footerTxt,
    },
    minify: true,
  },
  afterTest: (output) => {
    const chunk = output.output[0] as RolldownOutputChunk
    expect(chunk.code).toContain(bannerTxt)
    expect(chunk.code).toContain('// intro comment')
    expect(chunk.code).toContain('// outro comment')
    expect(chunk.code).toContain(footerTxt)
    // Verify they're in the right positions
    expect(chunk.code.startsWith(bannerTxt)).toBe(true)
    expect(chunk.code.endsWith(footerTxt)).toBe(true)
  },
})
