import { defineTest } from '@tests'
import { expect } from 'vitest'
import path from 'node:path'

export default defineTest({
  config: {
    external: (source: string, importer: string | undefined) => {
      expect(importer).toStrictEqual(path.join(__dirname, 'main.js'))
      if (source.startsWith('external')) {
        return true
      }
    },
  },
})
