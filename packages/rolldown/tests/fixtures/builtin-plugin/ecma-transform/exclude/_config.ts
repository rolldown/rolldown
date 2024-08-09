import { transformPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'
import { expect } from 'vitest'

let transformed: string[] = []
export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      transformPlugin({
        exclude: ['node_modules/**'],
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
    expect(transformed.filter((id) => id.includes('node_modules')).length).toBe(
      0,
    )
    transformed = []
  },
})
