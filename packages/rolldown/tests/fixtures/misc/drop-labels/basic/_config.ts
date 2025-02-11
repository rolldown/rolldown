import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    dropLabels: ['DROP'],
  },
  afterTest: (output) => {
    expect(output.output[0].code).not.toContain('DROP')
    expect(output.output[0].code).toContain('console.log')
  },
})
