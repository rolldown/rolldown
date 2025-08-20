import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      minify: {
        compress: {
          dropConsole: true
        }
      }
    }
  },
  afterTest: async (output) => {
    await import('./assert.mjs')
    expect( output.output[0].code).to.not.contain('console.log')
  },
})
