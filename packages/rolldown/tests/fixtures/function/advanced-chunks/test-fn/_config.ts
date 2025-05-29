import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    output: {
      advancedChunks: {
        groups: [
          {
            name: 'ab',
            test: (id) => {
              return /[ab]\.js/.test(id)
            },
          },
          {
            name: 'cd',
            test: /[cd]\.js/
          },
        ],
      },
    },
  },
  afterTest(output) {
    function findChunkStartWith(prefix: string) {
      return output.output.find(chunk => chunk.type === 'chunk' && chunk.fileName.startsWith(prefix));
    }
    const ab = findChunkStartWith('ab-')
    const cd = findChunkStartWith('cd-')

    if (ab?.type !== 'chunk' || cd?.type !== 'chunk') {
      throw new Error('should be chunk')
    }

    expect(ab.moduleIds).toMatchObject([
      /a.js$/,
      /b.js$/
    ])

    expect(cd.moduleIds).toMatchObject([
      /c.js$/,
      /d.js$/,
    ])
  },
})
