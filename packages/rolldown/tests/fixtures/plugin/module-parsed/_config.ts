import { defineTest } from '@tests'
import { expect, vi } from 'vitest'

const moduleParsedFn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        moduleParsed: function (moduleInfo) {
          moduleParsedFn()
          expect(moduleInfo.code).include(`module-parsed`)
          expect(moduleInfo.id.endsWith('main.js')).toBeTruthy()
        },
      },
    ],
  },
  afterTest: () => {
    expect(moduleParsedFn).toHaveBeenCalledTimes(1)
  },
})
