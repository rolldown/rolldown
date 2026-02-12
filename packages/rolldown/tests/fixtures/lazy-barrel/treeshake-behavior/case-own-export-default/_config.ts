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
    // import gg (default import) - own export `default` (which is `gg`) is used
    // `default` is barrel's own export (re-exported from imported `gg`), so barrel must be executed.
    // This should behave exactly the same as `import { index } from '../barrel'`.
    // When barrel executes, ALL its import records must be loaded because
    // sideEffects can only be determined after transform hook.
    // This includes both imports and re-exports.
    // However, d.js's re-export `export { dd } from './dd.js'` is not loaded
    // because `dd` is not requested by anyone.
    // g.js is loaded because barrel imports `gg` from it.
    // g.js is a pure re-export barrel, so gg.js is loaded to resolve `gg`.
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('../barrel/index.js')
    expect(relativeIds).toContain('../barrel/a.js')
    expect(relativeIds).toContain('../barrel/b.js')
    expect(relativeIds).toContain('../barrel/c.js')
    expect(relativeIds).toContain('../barrel/d.js')
    expect(relativeIds).toContain('../barrel/e.js')
    expect(relativeIds).toContain('../barrel/f.js')
    expect(relativeIds).toContain('../barrel/g.js')
    expect(relativeIds).toContain('../barrel/gg.js')
    expect(transformedIds.length).toBe(10)
  },
})
