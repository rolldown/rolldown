import { aliasPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      aliasPlugin({
        entries: [{ find: /\d+/, replacement: '' }],
      }),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
