import { defineTest } from '@tests'
import { TestKind } from '@tests/types'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform: function (code, id, meta) {
          if (id.endsWith('foo.js')) {
            expect(code).toStrictEqual('')
            expect(meta.moduleType).toEqual('js')
            return {
              code: `console.log('transformed')`,
            }
          }
        },
      },
      {
        name: 'test-2',
        transform() {
          return null
        },
      },
    ],
  },
  afterNormalizedOptions(testKind: TestKind, options) {
      expect(options).not.toBeUndefined()
    if (testKind === 'compose-js-plugin') {
      expect(options?.plugins.length).toBe(1)
    } else {

      expect(options?.plugins.length).toBe(2)
    }
  },
  afterTest: (output) => {
    expect(output.output[0].code).contains('transformed')
  },
})
