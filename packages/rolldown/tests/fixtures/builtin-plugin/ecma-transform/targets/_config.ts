import { transformPlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: './main.ts',
    plugins: [
      transformPlugin({
        exclude: ['node_modules/**'],
        browserslist: 'chrome 49',
      }),
    ],
  },
  async afterTest(src) {
    expect(src.output[0].code.includes('||=')).toBe(false)
  },
})
