import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { modulePreloadPolyfillPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    output: {
      format: 'cjs',
    },
    define: {
      __VITE_IS_MODERN__: 'false',
    },
    plugins: [modulePreloadPolyfillPlugin()],
  },
  async afterTest(output) {
    expect(output.output[0].code.length).toBe(0)
  },
})
