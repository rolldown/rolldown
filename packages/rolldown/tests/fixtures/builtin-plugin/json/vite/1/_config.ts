import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { jsonPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    input: 'main.js',
    plugins: [
      jsonPlugin({ namedExports: true, stringify: 'auto' }),
      {
        name: 'test-plugin',
        async transform(code, id) {
          if (id.endsWith('.json')) {
            await expect(code).toMatchFileSnapshot(
              path.resolve(import.meta.dirname, `${path.basename(id)}.snap`),
            )
          }
        },
      },
    ],
  }
})
