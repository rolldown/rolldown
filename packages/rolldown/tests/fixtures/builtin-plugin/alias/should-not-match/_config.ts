import { aliasPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      aliasPlugin({
        entries: [{ find: 'rolldown', replacement: '.' }],
      }),
    ],
  },
  // cspell:ignore rolldownlib
  async afterTest() {
    try {
      await import('./assert.mjs')
    } catch (err: any) {
      expect(err.toString()).contains(
        `Failed to load url rolldownlib.js (resolved id: rolldownlib.js)`,
      )
    }
  },
})
