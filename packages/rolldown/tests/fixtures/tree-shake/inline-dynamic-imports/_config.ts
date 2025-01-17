import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      inlineDynamicImports: true,
    },
  },
  afterTest: (output) => {
    expect(output.output.length).toEqual(1)
    expect(output.output[0].code).toContain('"b"')
  },
})
