import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { dynamicImportVarsPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    plugins: [
      dynamicImportVarsPlugin({
        async resolver(id) {
          return id
            .replace("@", path.resolve(import.meta.dirname, "./mods/"))
            .replace("#", path.resolve(import.meta.dirname, "../../"))
        },
      }),
    ],
  },
  async afterTest(output) {
    for (const chunk of output.output) {
      if (chunk.type === 'chunk') {
        await expect(chunk.code).toMatchFileSnapshot(
          path.resolve(import.meta.dirname, 'main.js.snap'),
        )
      }
    }
  },
})
