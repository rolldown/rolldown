import path from 'node:path'
import { expect } from 'vitest'
import { stripAnsi } from 'consola/utils'
import { defineTest } from 'rolldown-tests'
import { isolatedDeclarationPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    input: 'main.ts',
    plugins: [
      isolatedDeclarationPlugin(),
    ],
  },
  async catchError(err: any) {
    await expect(stripAnsi(err.toString())).toMatchFileSnapshot(
      path.resolve(import.meta.dirname, "main.ts.snap")
    )
  },
})
