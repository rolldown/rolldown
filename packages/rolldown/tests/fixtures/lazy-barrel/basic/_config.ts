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
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    )
    // With lazy barrel optimization, only 'a.js' should be loaded
    // 'b.js' should NOT be loaded since it's not imported
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel/index.js')
    expect(relativeIds).toContain('barrel/a.js')
    expect(transformedIds.length).toBe(3)
  },
})
