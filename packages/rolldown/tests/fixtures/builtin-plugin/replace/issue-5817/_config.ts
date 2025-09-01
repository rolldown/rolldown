import { replacePlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import {expect} from 'vitest'

export default defineTest({
  config: {
    plugins: [
      replacePlugin({
        // @ts-ignore
        __rolldown: 1
      }),
    ],
  },
  afterTest(output) {
    expect(output.output[0].code).toContain(`console.log(1)`)
 }
})
