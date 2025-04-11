import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { wasmFallbackPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    plugins: [wasmFallbackPlugin()],
  },
  catchError(err) {
    expect((err as Error).message).includes("[UNRESOLVED_IMPORT] Error")
  },
  afterTest() {
    expect.unreachable('wasmFallbackPlugin')
  }
})
