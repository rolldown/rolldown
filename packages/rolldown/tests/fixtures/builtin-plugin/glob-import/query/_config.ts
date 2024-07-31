import { BuiltinGlobImportPlugin } from 'rolldown'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    plugins: [new BuiltinGlobImportPlugin()],
  },
  async afterTest() {
    await import('./assert.mjs')
  },
})
