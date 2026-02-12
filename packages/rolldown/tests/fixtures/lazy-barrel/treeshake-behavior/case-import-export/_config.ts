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
    // import { d } - import-then-export pattern
    // Barrel has `import { d } from './d.js'; export { d };`
    // Only `d` is requested, so only d.js needs to be loaded from barrel.
    // Since `d` is d.js's own export, d.js must be executed.
    // When d.js executes, ALL its import records must be loaded,
    // including `export { dd } from './dd.js'`.
    // Barrel's other import records are skipped (not needed for `d`).
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('../barrel/index.js')
    expect(relativeIds).toContain('../barrel/d.js')
    expect(relativeIds).toContain('../barrel/dd.js')
    expect(transformedIds.length).toBe(4)
  },
})
