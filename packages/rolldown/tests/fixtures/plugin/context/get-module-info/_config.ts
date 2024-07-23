import { defineTest } from '@tests'
import { expect } from 'vitest'
import path from 'path'

const meta = { value: 1 }
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        resolveId() {
          return {
            id: path.join(__dirname, 'main.js'),
            meta,
          }
        },
        async renderStart() {
          const entry = path.join(__dirname, 'main.js')
          const moduleInfo = this.getModuleInfo(entry)!
          expect(moduleInfo.isEntry).toBe(true)
          expect(moduleInfo.id).toBe(entry)
          expect(moduleInfo.meta).toBe(meta)
        },
      },
    ],
  },
})
