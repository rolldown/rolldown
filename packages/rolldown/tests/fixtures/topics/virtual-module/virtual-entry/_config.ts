// cSpell:disable
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: 'test',
    plugins: [
      {
        name: 'virtual-module',
        resolveId(id) {
          if (id === 'test') {
            return '\0virtual:test'
          }
        },
        load(id) {
          if (id === '\0virtual:test') {
            return 'export default "test"'
          }
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore Ther will be a MODULE NOT FOUND error before the test is executed
    const exports = await import('./dist/virtualtest.js')
    expect(exports.default).toBe('test')
  },
})
