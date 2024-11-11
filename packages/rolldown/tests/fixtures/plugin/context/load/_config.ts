import { defineTest } from '@tests'
import { expect } from 'vitest'
import path from 'node:path'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async load(id) {
          if (id.endsWith('main.js')) {
            const moduleInfo = await this.load({
              id: path.join(__dirname, 'foo.js'),
            })
            expect(moduleInfo.code!.includes('foo')).toBe(true)
          }
        },
      },
    ],
  },
})
