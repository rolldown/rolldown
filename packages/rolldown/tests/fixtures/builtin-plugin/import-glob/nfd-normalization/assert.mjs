import assert from 'node:assert'
import { modules } from './dist/main'

// The key should use the NFC form (as written in source code),
// regardless of how the filesystem stores the directory name.
const nfcKey = './\u30DD/a.js'

assert.ok(modules[nfcKey], `Expected glob to match NFD directory with NFC pattern. Keys: ${JSON.stringify(Object.keys(modules))}`)

modules[nfcKey]().then((m) => {
  assert.strictEqual(m.default, 'a')
})
