import { assetPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import path from 'path'

export default defineTest({
  config: {
    plugins: [
      assetPlugin({}),
    ],
  },
  async afterTest(output) {
    await expect(output.output[0].code).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'main.js.snap')
    )
  },
})
