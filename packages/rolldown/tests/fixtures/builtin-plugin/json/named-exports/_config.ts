import { jsonPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      jsonPlugin({ stringify: false, isBuild: true, namedExports: true }),
    ],
  },
  async afterTest(output) {
    expect(output.output[0].code).toContain(`const name = "stringify";`)
    await import('./assert.mjs')
  },
})
