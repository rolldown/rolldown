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
            moduleSideEffects: id.includes('non-barrel')
          }
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    )
    // With lazy barrel optimization:
    // - main.js: entry point, all imports and re-exports are loaded even if side-effect-free
    // - barrel: only 'a.js' is loaded (b.js is skipped due to side-effect-free)
    // - non-barrel: all modules are loaded (has side effects)
    // - other.js: loaded because it's re-exported from entry
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel/index.js')
    expect(relativeIds).toContain('barrel/a.js')
    expect(relativeIds).toContain('non-barrel/index.js')
    expect(relativeIds).toContain('non-barrel/c.js')
    expect(relativeIds).toContain('non-barrel/d.js')
    expect(relativeIds).toContain('other.js')
    expect(transformedIds.length).toBe(7)
  },
})
