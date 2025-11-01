import assert from 'node:assert'
import { value as conditionalValue } from 'conditional-exports-lib'
import { util } from 'subpath-exports-lib/utils'
import { helper } from 'nested-conditions-lib'

// Test conditional exports resolution
assert.strictEqual(conditionalValue, 'import-condition')

// Test subpath exports
assert.strictEqual(util, 'utility-function')

// Test nested conditional exports - uses browser by default in rolldown
assert.strictEqual(helper, 'browser-helper-value')
