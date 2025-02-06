import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import fs from 'node:fs'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'chunk-url',
        load(id) {
          // similar to vite:load-fallback
          if (id.endsWith('?chunk-url')) {
            return fs.readFileSync(id.replace('?chunk-url', ''), 'utf-8')
          }
        },
        transform(_code, id) {
          // replace `?chunk-url` module with a reference to the chunk
          if (id.endsWith('?chunk-url')) {
            const referenceId = this.emitFile({
              type: 'chunk',
              id: id.replace('?chunk-url', ''),
            })
            return `export default import.meta.ROLLUP_FILE_URL_${referenceId}`
          }
        },
      },
    ],
  },
  afterTest: async () => {
    const main = await import('./dist/main.js' as string)
    const depUrl = new URL(
      main.default,
      new URL('./dist/main.js', import.meta.url),
    ).href
    const dep = await import(depUrl)
    expect(dep.default).toBe('dep.js')
  },
})
