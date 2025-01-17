import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import path from 'node:path'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context-load-same-id-concurrent',
        async transform(code, id) {
          if (id.endsWith('main.js')) {
            const promises = [
              path.join(__dirname, 'foo.js'),
              path.join(__dirname, 'foo.js'),
            ].map(async (id) => {
              return await this.load({ id })
            })
            const result = await Promise.all(promises)
            expect(result[0].code!.includes('foo')).toBe(true)
          }
        },
      },
    ],
  },
})
