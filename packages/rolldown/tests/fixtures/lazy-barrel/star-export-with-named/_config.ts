import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const transformedIds: string[] = []

export default defineTest({
  skip: true,
  config: {
    // experimental: {
    //   lazyBarrel: true,
    // },
    plugins: [
      {
        name: 'track-transforms',
        transform(_, id) {
          transformedIds.push(id);
          return {
            moduleSideEffects: false
          }
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    )
    // Barrel has both named export `export { a }` and star export `export *`
    // Since `a` is found in named exports, no need to search star exports
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel/index.js')
    expect(relativeIds).toContain('barrel/a.js')
    expect(transformedIds.length).toBe(3)
  },
})
