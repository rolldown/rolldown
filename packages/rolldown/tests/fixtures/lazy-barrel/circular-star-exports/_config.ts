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
    // Circular star exports: barrel-a `export * from barrel-b`, barrel-b `export * from barrel-a`
    // main.js imports `b` which is not in barrel-a's named exports
    // So barrel-b/index.js must be loaded to find `b` in star exports
    // `b` is found via barrel-b's `export * from './b'`
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel-a/index.js')
    expect(relativeIds).toContain('barrel-b/index.js')
    expect(relativeIds).toContain('barrel-b/b.js')
    expect(transformedIds.length).toBe(4)
  },
})
