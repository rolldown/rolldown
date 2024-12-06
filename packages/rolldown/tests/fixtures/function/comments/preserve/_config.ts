import nodePath from 'node:path'
import { expect } from 'vitest'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    output: {
      comments: 'preserve',
    },
  },
  afterTest: function (output) {
    expect(output.output[0].code).toMatchFileSnapshot(
      nodePath.join(import.meta.dirname, 'output.snap'),
    )
  },
})
