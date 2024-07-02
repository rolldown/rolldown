import { defineTest } from '@tests'
import { expect, vi } from 'vitest'
import path from 'path'

const fn = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async renderStart() {
          const entry = path.join(__dirname, 'main.js')
          const moduleInfo = this.getModuleInfo(entry)!
          expect(moduleInfo.isEntry).toBe(true)
          expect(moduleInfo.id).toBe(entry)
        },
      },
    ],
  },
})
