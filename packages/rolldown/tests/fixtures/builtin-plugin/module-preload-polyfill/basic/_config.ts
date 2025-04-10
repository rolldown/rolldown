import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { modulePreloadPolyfillPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    plugins: [modulePreloadPolyfillPlugin()],
  },
  async afterTest(output) {
    await expect(output.output[0].code).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, 'main.js.snap'),
    )
  },
})
