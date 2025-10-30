import assert from 'node:assert'
import { value } from 'browser-lib'

// Test that browser field is respected when platform is browser
assert.strictEqual(value, 'browser-implementation')
