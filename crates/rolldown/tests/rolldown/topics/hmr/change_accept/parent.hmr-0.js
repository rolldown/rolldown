import assert from 'node:assert'
import './child.js'

import.meta.hot.accept('./child.js', () => {
  globalThis.newAcceptWasCalled = true
})

process.on('beforeExit', (code) => {
  if (code !== 0) return
  assert(globalThis.newAcceptWasCalled)
})
