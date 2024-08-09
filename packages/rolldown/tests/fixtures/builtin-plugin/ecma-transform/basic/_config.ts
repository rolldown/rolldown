import { transformPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      transformPlugin(),
      {
        name: 'test',
        transform(code, id) {
          // after transform there should be no `interface`
          expect(code).not.include('interface')
          return null
        },
      },
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
