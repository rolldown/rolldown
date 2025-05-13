import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

import { getOutputChunkNames } from 'rolldown-tests/utils'

export default defineTest({
  config: {
    input: ['main.js'],
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
            return `export default 'module'`
          }
        },
      },
    ],
    output: {
      preserveModules: true,
      virtualDirname: 'custom-virtual',
    }
  },
  // cSpell:disable
  afterTest(output) {
    if (process.platform !== 'win32') {
      expect(getOutputChunkNames(output)).toStrictEqual([
        'main.js',
        'custom-virtual/_module.js',
      ])
    }
  },
})
