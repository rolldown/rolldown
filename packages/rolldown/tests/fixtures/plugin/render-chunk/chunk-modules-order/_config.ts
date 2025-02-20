import { expect } from 'vitest'
import path from 'node:path'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        renderChunk: (_code, chunk) => {
          let keys = Object.keys(chunk.modules).map((item) => {
            return path.basename(item)
          })
          expect(keys).toEqual([
            'aa.js',
            'ab.js',
            'a.js',
            'ba.js',
            'bb.js',
            'b.js',
            'main.js',
          ])
        },
      },
    ],
  },
})
