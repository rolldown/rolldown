import { jsonPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [jsonPlugin({ stringify: true, minify: true })],
  },
  async afterTest(output) {
    expect(output.output[0].code).toContain(
      `JSON.parse("{\\"name\\":\\"@test-fixture/build\\"`,
    )
    await import('./assert.mjs')
  },
})
