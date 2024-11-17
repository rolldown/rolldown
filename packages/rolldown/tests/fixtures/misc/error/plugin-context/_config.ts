import { defineTest } from '@tests'
import { join } from 'node:path'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'my-plugin',
        async transform(_code, id) {
          if (id.includes('main.js')) {
            return this.error('my-error', 4)
          }
        },
      },
    ],
  },
  catchError(e: any) {
    expect(e.message).toContain('[plugin my-plugin]')
    expect(e.message).toContain(`\
[plugin my-plugin] ${join(import.meta.dirname, 'main.js')}:2:0
RollupError: my-error
1: xxx
2: yyy
   ^
3: zzz
`)
    expect(e.errors[0]).toMatchObject({
      message: 'my-error',
      code: 'PLUGIN_ERROR',
      plugin: 'my-plugin',
      hook: 'transform',
    })
  },
})
