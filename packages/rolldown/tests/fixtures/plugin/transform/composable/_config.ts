import { defineTest } from '@tests'
import { expect } from 'vitest'
import { composeJsPlugins } from 'rolldown/experimental'
import { Plugin as RolldownPlugin } from 'rolldown'

let plugins: RolldownPlugin[] = [
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
]

export default defineTest({
  config: {
    plugins,
  },
  beforeTest(testKind) {
    if (testKind === 'compose-js-plugin') {
      plugins = composeJsPlugins(plugins) as RolldownPlugin[]
      expect(plugins.length).toBe(1)
    } else {
      expect(plugins.length).toBe(2)
    }
  },
  afterTest: (output) => {
    expect(output.output[0].code).contains('transformed')
  },
})
