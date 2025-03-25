import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { jsonPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    input: 'main.js',
    plugins: [
      jsonPlugin({ namedExports: false, stringify: true, isBuild: true }),
      {
        name: 'test-plugin',
        async transform(code, id) {
          if (id.endsWith('data.json')) {
            await expect(code).toMatchFileSnapshot(
              path.resolve(import.meta.dirname, 'data.json.snap'),
            )
          }
        },
      },
    ],
  },
})
