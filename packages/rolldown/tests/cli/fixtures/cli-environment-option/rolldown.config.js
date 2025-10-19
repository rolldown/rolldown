import { defineConfig } from 'rolldown'
import assert from 'node:assert'

export default defineConfig(() => {
  // Check that environment variables are set correctly
  assert.strictEqual(process.env.PRODUCTION, 'true')
  assert.strictEqual(process.env.FOO, 'bar')
  assert.strictEqual(process.env.HOST, 'http://localhost:4000')
  return {
    input: './index.js'
  }
})
