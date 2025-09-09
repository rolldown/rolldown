import { defineConfig } from 'rolldown'
import assert from 'node:assert'

export default defineConfig((args) => {
  assert.strictEqual(args.customArg, 'customValue')
  return {
    input: './index.js'
  }
})
