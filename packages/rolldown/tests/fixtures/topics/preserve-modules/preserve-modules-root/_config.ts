import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { getOutputChunkNames } from '../../../../src/utils'


export default defineTest({
  config: {
    input: ['src/index.js'],
    output: {
      preserveModules: true,
      preserveModulesRoot: 'src',
    }
  },
  afterTest: (output) => {
    if (process.platform !== 'win32') {
      expect(getOutputChunkNames(output)).toStrictEqual([
        'index.js',
        'package.json.js',
        'utils/index.js'
      ])
    }
  },
})

