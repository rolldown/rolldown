import { BuiltinDynamicImportVarsPlugin, BuiltinGlobImportPlugin } from 'rolldown'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [
      new BuiltinDynamicImportVarsPlugin(),
      new BuiltinGlobImportPlugin(),
    ],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
