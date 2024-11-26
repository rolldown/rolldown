import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async load(id) {
          try {
            await this.load({
              id,
            })
          } catch (e: any) {
            expect(e.message).toContain('cycle loading')
          }
        },
      },
    ],
  },
})
