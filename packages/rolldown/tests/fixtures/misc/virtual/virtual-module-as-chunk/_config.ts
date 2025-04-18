import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const fn = vi.fn()
import { getOutputChunkNames } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: ['main.js', 'entry.js'],
    plugins: [
      {
        name: 'virtual-module',
        resolveId(id) {
          if (id === '\0module') {
            return id
          }
        },
        load(id) {
          if (id === '\0module') {
            fn()
            return `export default 'module'`
          }
        },
      },
    ],
  },
  afterTest(output) {
    // cSpell:disable
    expect(getOutputChunkNames(output)).toStrictEqual([
      'entry.js',
      'main.js',
      '_module-iGCt1YOi.js',
    ])
  },
})
