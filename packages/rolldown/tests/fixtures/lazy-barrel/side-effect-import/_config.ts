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
          if (!id.endsWith('main.js')) {
            return {
              moduleSideEffects: false
            }
          }
        },
      },
    ],
  },
  afterTest: () => {
    const relativeIds = transformedIds.map((id) =>
      path.relative(import.meta.dirname, id).replace(/\\/g, '/'),
    )
    // Side-effect import: main.js does `import './barrel'`
    // barrel/index.js has `import { b }`, so b.js is loaded
    // b.js has `export { c }` but c is not used, so c.js should NOT be loaded
    // a.js is only re-exported, not used in side-effect import
    expect(relativeIds).toContain('main.js')
    expect(relativeIds).toContain('barrel/index.js')
    expect(relativeIds).toContain('barrel/b.js')
    expect(transformedIds.length).toBe(3)
  },
})
