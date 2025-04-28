import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'
import path from 'node:path'

export default defineTest({
  config: {
    input: ['main.js', 'foo.js'],
    plugins: [
      {
        name: 'test-plugin-context',
        async transform(code, id) {
          if (id.endsWith('main.js')) {
            const moduleInfo = await this.load({
              id: path.join(__dirname, 'foo.js'),
            })
            expect(moduleInfo.code!.includes('foo')).toBe(true)
          }
          if (id.endsWith('foo.js')) {
            await new Promise((resolve) => {
              setTimeout(resolve, 10);
            })
          }
        },
      },
    ],
  }
})
