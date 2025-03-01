import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    output: {
      file: 'dist/main.js',
      target: 'es2015',
      banner: '// banner',
    },
    plugins: [
      {
        name: 'test-plugin',
        outputOptions: function (options) {
          expect(options.file).toBe('dist/main.js')
          expect(options.target).toBe('es2015')
          expect(options.banner).toBe('// banner')
        },
      },
    ],
  },
})
