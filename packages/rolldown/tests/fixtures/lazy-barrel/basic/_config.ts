import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const transformedIds: string[] = []

export default defineTest({
  config: {
    input: './src/main.js',
    experimental: {
      lazyBarrel: true,
    },
    plugins: [
      {
        name: 'track-transforms',
        transform(_, id) {
          // Skip virtual modules (like \0rolldown/runtime.js)
          if (id.startsWith('\0')) {
            return;
          }
          transformedIds.push(id);
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
    expect(relativeIds).toContain('src/main.js')
    expect(relativeIds).toContain('src/barrel/index.js')
    expect(relativeIds).toContain('src/barrel/a.js')
    expect(relativeIds).toContain('src/non-barrel/index.js')
    expect(relativeIds).toContain('src/non-barrel/c.js')
    expect(relativeIds).toContain('src/non-barrel/d.js')
    expect(relativeIds).toContain('src/other.js')
    expect(transformedIds.length).toBe(7)
  },
})
