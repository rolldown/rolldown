import { transformPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

let transformed: string[] = []
export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      transformPlugin({
        exclude: ['**/node_modules/**'],
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
    // expect(transformed.splice(0).filter((id) => id.includes('node_modules')).length).toBe(
    //   0,
    // )
  },
})
