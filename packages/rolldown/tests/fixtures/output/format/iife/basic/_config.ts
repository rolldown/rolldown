import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      format: 'iife',
      name: 'myModule',
    },
  },
  afterTest: (output) => {
    // In rollup, the without any input, the output is an empty IIFE, without the assignment.
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function() {


      })();"
    `)
  },
})
