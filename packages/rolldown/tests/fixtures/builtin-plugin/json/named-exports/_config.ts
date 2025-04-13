import { jsonPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      jsonPlugin({ stringify: false, minify: true, namedExports: true }),
    ],
  },
  async afterTest(output) {
    expect(output.output[0].code).toContain(
      `const name = "@test-fixture/named-exports";`,
    )
    await import('./assert.mjs')
  },
})
