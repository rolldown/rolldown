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
    // Circular exports: barrel-a has `export * from barrel-b`, barrel-b has `export { a as c } from barrel-a`
    // main.js imports `c` which is not in barrel-a's named exports
    // So barrel-b/index.js must be loaded to find `c` in star exports
    // `c` resolves to barrel-a's `a`, so barrel-a/a.js is loaded
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel-a/index.js')
    expect(relativeIds).toContain('barrel-b/index.js')
    expect(relativeIds).toContain('barrel-a/a.js')
    expect(transformedIds.length).toBe(4)
  },
})
