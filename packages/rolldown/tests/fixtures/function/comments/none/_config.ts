import nodePath from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    output: {
      comments: 'none',
    },
  },
  afterTest(output) {
    expect(output.output[0].code).toMatchFileSnapshot(
      nodePath.join(import.meta.dirname, 'output.snap'),
    )
  },
})
