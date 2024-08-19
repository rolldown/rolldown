import { jsonPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [jsonPlugin({ stringify: true })],
  },
  async afterTest(output) {
    expect(output.output[0].code).toContain('JSON.parse')
    await import('./assert.mjs')
  },
})
