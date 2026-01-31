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
    // main.js imports `b` from barrel.
    // index.js has `export { a as b } from './index'` (self-reference).
    // This resolves to index.js's `export { a } from './a'`, which loads a.js.
    // Since `a` is a.js's own export (not a re-export), a.js must be executed,
    // causing all its import records to be loaded, including `export { b } from './b'`.
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel/index.js')
    expect(relativeIds).toContain('barrel/a.js')
    expect(relativeIds).toContain('barrel/b.js')
    expect(transformedIds.length).toBe(4)
  },
})
