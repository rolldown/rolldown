import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

const bannerTxt = '// ==UserScript==\n// @name My Script\n// @version 1.0.0\n// ==/UserScript=='
const footerTxt = '// Build timestamp: 2024-01-01'

export default defineTest({
  config: {
    output: {
      banner: bannerTxt,
      intro: '// Intro: Code starts here',
      outro: '// Outro: Code ends here',
      footer: footerTxt,
    },
    minify: true,
  },
  afterTest: (output) => {
    const chunk = output.output[0] as RolldownOutputChunk
    const code = chunk.code
    
    // Banner and footer should be preserved
    expect(code).toContain(bannerTxt)
    expect(code).toContain('// Intro: Code starts here')
    expect(code).toContain('// Outro: Code ends here')
    expect(code).toContain(footerTxt)
    
    // Regular comments should be removed by minification
    expect(code).not.toContain('This is a regular comment')
    expect(code).not.toContain('Another comment')
    expect(code).not.toContain('Block comment')
    
    // Verify correct positioning
    const lines = code.split('\n')
    expect(lines[0]).toBe('// ==UserScript==')
    expect(lines[lines.length - 1]).toBe(footerTxt)
  },
})
