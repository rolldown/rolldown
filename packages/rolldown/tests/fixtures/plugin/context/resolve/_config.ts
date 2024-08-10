import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import nodePath from 'path'

const fn = vi.fn()

export default defineTest({
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
          expect(props).toMatchInlineSnapshot(`
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
