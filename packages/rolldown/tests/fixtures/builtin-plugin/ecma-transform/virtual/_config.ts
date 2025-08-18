import { defineTest } from 'rolldown-tests'
import { transformPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    input: './main.js',
    plugins: [
      transformPlugin(),
      {
        name: 'virtual',
        resolveId(source) {
          if (source === 'virtual:test.tsx') {
            return "\0" + source
          }
        },
        load(id) {
          if (id === "\0virtual:test.tsx") {
            // this module should be skipped by builtin transform
            // otherwise this will cause a syntax error
            // or mysterious tsconfig error
            // > Tsconfig extends configs circularly: "tsconfig.json" -> "tsconfig.json"
            return `bad code`;
          }
        },
        transform(code, id) {
          if (id === "\0virtual:test.tsx") {
            return `export default "fixed"`;
          }
        }
      },
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
