import { transformPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

let transformed: string[] = []

// The test is valid, since we process none js by default
// The only thing we need to track file that has `moduleType` as `js`
export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      transformPlugin({
        include: ['**/node_modules/**'],
      }),
      {
        name: 'test',
        transform(_, id, meta) {
          if (meta.moduleType === 'js') {
            transformed.push(id)
          }
          return null
        },
      },
    ],
  },
  async afterTest() {
    expect(transformed.length).toBe(1)
    expect(transformed.filter((id) => id.includes('node_modules')).length).toBe(
      1,
    )
    transformed = []
  },
})
