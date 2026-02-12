import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const transformedIds: string[] = []

export default defineTest({
  config: {
    input: ['./1.js', './2.js', './3.js'],
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
    // Multiple entries with circular barrel exports:
    // 1.js imports `a` from barrel-a -> barrel-a/a.js
    // 2.js imports `b` from barrel-b -> barrel-b/b.js
    // 3.js imports `c` from barrel-c -> barrel-c re-exports `d as c` from barrel-b
    //   -> `d` is from barrel-a's `export { b as d } from barrel-b`
    //   -> eventually resolves to barrel-b/b.js
    expect(relativeIds).toContain('1.js')
    expect(relativeIds).toContain('2.js')
    expect(relativeIds).toContain('3.js')
    expect(relativeIds).toContain('barrel-a/index.js')
    expect(relativeIds).toContain('barrel-a/a.js')
    expect(relativeIds).toContain('barrel-b/index.js')
    expect(relativeIds).toContain('barrel-b/b.js')
    expect(relativeIds).toContain('barrel-c.js')
    expect(transformedIds.length).toBe(8)
  },
})
