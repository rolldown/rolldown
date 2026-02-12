import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const transformedIds: string[] = []

// barrel/other.js, barrel/c.js and barrel/f.js are marked as no side effects
const noSideEffectsPattern = /barrel[\\/](other|c|f)\.js$/

export default defineTest({
  config: {
    experimental: {
      lazyBarrel: true,
    },
    treeshake: {
      moduleSideEffects(id) {
        if (noSideEffectsPattern.test(id)) {
          return false
        }
        return true
      },
    },
    plugins: [
      {
        name: 'track-transforms',
        transform(_, id) {
          // Skip virtual modules (like \0rolldown/runtime.js)
          if (id.startsWith('\0')) {
            return;
          }
          transformedIds.push(id)
          if (id.endsWith('d.js') || id.endsWith('g.js')) {
            return { moduleSideEffects: false }
          }
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    )
    // import gg (default import) - `export { gg as default }` is a re-export
    // Unlike `export default gg`, this is NOT an own export.
    // Only g.js and gg.js need to be loaded to resolve `default`.
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('../barrel/other.js')
    expect(relativeIds).toContain('../barrel/g.js')
    expect(relativeIds).toContain('../barrel/gg.js')
    expect(transformedIds.length).toBe(4)
  },
})
