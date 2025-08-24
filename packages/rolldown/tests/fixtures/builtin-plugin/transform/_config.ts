import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { transformPlugin } from 'rolldown/experimental'
import { stripAnsi } from 'consola/utils'

export default defineTest({
  config: {
    plugins: [
      transformPlugin({
        jsxRefreshInclude: [/.abc$/],
        transformOptions: {
          jsx: {
            throwIfNamespace: true,
          },
        },
      }),
    ],
  },
  async catchError(err: any) {
    await expect(stripAnsi(err.toString())).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, "main.js.snap")
    )
  },
})
