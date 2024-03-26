import type { RollupOptions } from 'rolldown'
import { expect } from 'vitest'
import path from 'node:path'

const config: RollupOptions = {
  external: (
    source: string,
    importer: string | undefined,
    isResolved: boolean,
  ) => {
    expect(importer).toStrictEqual(path.join(__dirname, 'main.js'))
    if (source.startsWith('external')) {
      return true
    }
  },
}

export default {
  config,
}
