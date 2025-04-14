
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'
import { wasmHelperPlugin } from 'rolldown/experimental'

export default defineTest({
  config: {
    plugins: [wasmHelperPlugin()],
  },
  async afterTest(output) {
    expect(output.output[1].fileName).toBe('assets/add-Bodj1WnG.wasm')
    expect(output.output[0].modules['\0vite/wasm-helper.js']).toBeDefined()
    expect(Object.keys(output.output[0].modules).find(v => v.endsWith('add.wasm?init'))).toBeDefined()
  }
})
