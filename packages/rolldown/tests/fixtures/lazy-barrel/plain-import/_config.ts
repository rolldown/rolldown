import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const transformedIds: string[] = []

export default defineTest({
  config: {
    experimental: {
      lazyBarrel: true,
    },
    plugins: [
      {
        name: 'track-transforms',
        transform(_, id) {
          transformedIds.push(id)
          return {
            moduleSideEffects: id.replaceAll('\\', '/').includes('barrel/e.js'),
          }
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    )
    // When the imported module is side-effect-free, plain imports are skipped
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel/index.js')
    expect(relativeIds).toContain('barrel/d.js')
    expect(relativeIds).toContain('barrel/e.js')
    expect(transformedIds.length).toBe(4)
  },
})
