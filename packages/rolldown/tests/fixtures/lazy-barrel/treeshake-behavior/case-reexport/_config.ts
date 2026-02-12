import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const transformedIds: string[] = []

// barrel/index.js, barrel/c.js and barrel/f.js are marked as no side effects
const noSideEffectsPattern = /barrel[\\/](index|c|f)\.js$/

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
    // import { a } - only re-export `a` is used
    // `a` is re-exported from a.js, which has own export, so a.js must be executed.
    // Barrel's other import records are skipped (not needed for `a`).
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('../barrel/index.js')
    expect(relativeIds).toContain('../barrel/a.js')
    expect(transformedIds.length).toBe(3)
  },
})
