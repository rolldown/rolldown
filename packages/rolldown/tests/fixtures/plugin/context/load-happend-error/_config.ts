import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import path from 'node:path'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context-load-happend-error',
        load(id) {
          if (id.endsWith('foo.js')) {
            throw new Error('load foo.js error')
          }
        },
        async transform(code, id) {
          if (id.endsWith('main.js')) {
            await this.load({
              id: path.join(__dirname, 'foo.js'),
              moduleSideEffects: false,
            })
          }
        },
      },
    ],
  },
  catchError: (err: any) => {
    expect(err.message).toMatch(
      'load foo.js error'
    )
  }
})
