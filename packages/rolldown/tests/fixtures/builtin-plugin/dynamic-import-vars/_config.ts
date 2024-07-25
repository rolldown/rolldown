import { BuiltinDynamicImportVarsPlugin } from 'rolldown'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [new BuiltinDynamicImportVarsPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
