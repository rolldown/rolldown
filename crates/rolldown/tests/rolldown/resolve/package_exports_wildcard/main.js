import assert from 'node:assert'
import { feature1 } from 'wildcard-lib/features/feature1'
import { feature2 } from 'wildcard-lib/features/feature2'
import { utilA } from 'wildcard-lib/utils/utilA'

// Test wildcard pattern in package exports
assert.strictEqual(feature1, 'feature-one')
assert.strictEqual(feature2, 'feature-two')
assert.strictEqual(utilA, 'util-a')
