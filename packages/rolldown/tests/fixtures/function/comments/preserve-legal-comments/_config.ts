import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import nodePath from 'node:path'

export default defineTest({
  config: {
    output: {
      comments: 'preserve-legal',
    },
  },
  afterTest: function (output) {
    expect(output.output[0].code).toMatchFileSnapshot(
      nodePath.join(import.meta.dirname, 'output.snap'),
    )
  },
})
