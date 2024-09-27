import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import nodePath from 'node:path'

const fn = vi.fn()
let isComposingJs = false
export default defineTest({
  beforeTest(testKind) {
    isComposingJs = testKind === 'compose-js-plugin'
  },
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart(this) {
          const ret = await this.resolve('./main.js')
          if (!ret) {
            throw new Error('resolve failed')
          }
          const { id, ...props } = ret
          isComposingJs
            ? expect(props).toMatchInlineSnapshot(`
            {
              "external": false,
            }
          `)
            : expect(props).toMatchInlineSnapshot(`
            {
              "external": false,
            }
          `)
          expect(id).toEqual(nodePath.join(import.meta.dirname, 'main.js'))
          fn()
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1)
  },
})
