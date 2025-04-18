import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { transformPlugin } from 'rolldown/experimental'


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
    expect(transformed.splice(0)[0].includes('node_modules')).toBeTruthy()
  },
})
